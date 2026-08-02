//! Phase 1 of the [inline AST architecture]: build the inline tree as an
//! **internal, non-public artifact** and prove that folding it back to HTML
//! reproduces the existing `rendered()` string **byte-for-byte**.
//!
//! This is the "Strategy A" bring-up oracle described in the design document
//! (§4.1, §5.2, §5.4): the existing substitution pipeline is re-run with a
//! *marker-recording* renderer ([`RecordingRenderer`]) that wraps the built-in
//! [`HtmlSubstitutionRenderer`]. The recorder is a **transparent** decorator –
//! it writes the exact HTML the built-in renderer would, and additionally
//! brackets each recognized construct with inert Private-Use-Area sentinels.
//! Because the only bytes it adds are sentinels, two facts follow directly:
//!
//! 1. Stripping the sentinels from the recorded string yields the original
//!    `rendered()` output byte-for-byte (the *no-perturbation* invariant).
//! 2. Parsing the sentinel structure recovers a tree of [`InlineNode`]s, and
//!    folding that tree (text verbatim, leaves by their captured HTML,
//!    containers as open + folded children + close) reconstructs the same
//!    bytes.
//!
//! Nothing public changes: the recorder lives entirely in the test build and
//! `Content::rendered()` remains authoritative. Later phases promote the tree
//! to the canonical representation and make `rendered()` a fold over it.
//!
//! # These sentinels are temporary
//!
//! The `MARK_OPEN` / `MARK_CLOSE` / PUA-digit sentinels introduced below exist
//! *only* for this Strategy A oracle. The design is explicit that Strategy A is
//! not the end state (§4.1): the Phase 4 single-pass builder (Strategy B)
//! constructs nodes directly as the substitution sink, so there is no recording
//! pass and no markers at all. These sentinels are therefore retired wholesale
//! along with the recorder, and – unlike the three production sentinel systems
//! the node model also retires (passthrough / xref / footnote, §4.2) – they
//! never touch shipped output even now, since this module is test-only.
//!
//! A missing construct degrades gracefully. If a `render_*` method is not
//! intercepted, its output flows to the working string with no sentinels and is
//! recovered as ordinary [`Text`](InlineNode::Text); byte parity is preserved
//! regardless, so coverage can grow without ever risking the oracle.
//!
//! [inline AST architecture]: ../../../docs/design/inline-ast-architecture.md

// TEMPORARY: some node metadata is captured but not yet read while this is a
// bring-up oracle; the allow goes away when the tree becomes canonical
// (Phase 2) and every field is consumed.
#![allow(dead_code)]

use std::{cell::RefCell, rc::Rc};

use crate::{
    Parser, Span,
    attributes::Attrlist,
    blocks::FindBlocks,
    content::{Content, SubstitutionGroup},
    inlines::{
        Anchor, Callout, CharRef, Footnote, Image, IndexTerm, InlineNode, Ref, RefVariant,
        SpanForm, Stem, StemNotation, StyleVariant, Styled, Ui, UiKind,
    },
    parser::{
        CalloutRenderParams, FootnoteRenderParams, HtmlSubstitutionRenderer, IconRenderParams,
        ImageRenderParams, IndexTermRenderParams, InlineSubstitutionRenderer, LinkRenderParams,
        MenuRenderParams, ModificationContext, QuoteScope, QuoteType, XrefRenderParams,
    },
    strings::CowStr,
};

// ─── Sentinels ────────────────────────────────────────────────────────────
//
// All in the Unicode Private Use Area, chosen to be inert to every step's
// regexes, exactly as the existing pipeline's xref (`\u{E000}`) and passthrough
// (`\u{96}`) sentinels already are. Digits are encoded as PUA characters too,
// so a recorded marker injects no ASCII word characters that a later step could
// key off of.

/// Opens a recorded construct; immediately followed by the PUA-encoded index of
/// its [`Event`].
const MARK_OPEN: char = '\u{E010}';

/// Closes a recorded construct.
const MARK_CLOSE: char = '\u{E011}';

/// Base of the ten PUA "digit" characters (`\u{E020}`..=`\u{E029}`) used to
/// encode an [`Event`] index inside a marker.
const PUA_DIGIT_BASE: u32 = 0xe020;

/// A transient placeholder spliced in as a [quote substitution]'s body so the
/// fixed open/close wrapper can be recovered by splitting on it. It never
/// appears in real content.
///
/// [quote substitution]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
const SPLIT_PLACEHOLDER: char = '\u{E0FF}';

fn is_pua_digit(c: char) -> bool {
    let n = c as u32;
    (PUA_DIGIT_BASE..PUA_DIGIT_BASE + 10).contains(&n)
}

fn write_index(index: usize, dest: &mut String) {
    for ch in index.to_string().chars() {
        let digit = ch as u32 - '0' as u32;
        if let Some(pua) = char::from_u32(PUA_DIGIT_BASE + digit) {
            dest.push(pua);
        }
    }
}

fn is_marker(c: char) -> bool {
    c == MARK_OPEN || c == MARK_CLOSE || is_pua_digit(c)
}

/// Removes every recorder sentinel from `chars`, leaving the real (HTML) bytes.
fn strip_markers(chars: &[char]) -> String {
    chars.iter().copied().filter(|c| !is_marker(*c)).collect()
}

// ─── Recorded events ────────────────────────────────────────────────────────

/// The metadata captured for one recognized construct, looked up by the index
/// encoded in its [`MARK_OPEN`] marker.
#[derive(Clone, Debug)]
enum Event {
    /// A parent construct (a [quote substitution]). `open`/`close` are the
    /// fixed wrapper bytes the built-in renderer emits around the body.
    ///
    /// [quote substitution]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
    Container {
        open: String,
        close: String,
        node: ContainerNode,
    },

    /// A leaf construct. Its HTML is recovered from the marked string, so only
    /// the structural metadata is stored here.
    Leaf(LeafNode),
}

#[derive(Clone, Debug)]
enum ContainerNode {
    Styled {
        variant: StyleVariant,
        form: SpanForm,
        id: Option<String>,
        roles: Vec<String>,
    },
    Reference {
        variant: RefVariant,
        target: String,
        roles: Vec<String>,
        window: Option<String>,
    },
    Stem {
        notation: StemNotation,
    },
}

/// The recovered kind of a character reference. Special characters and
/// character replacements are *not* marked by the recorder (their escaped
/// output is re-consumed by later steps, so bracketing it would perturb
/// recognition); instead they are recovered by splitting text runs on the
/// entities the built-in renderer emits.
#[derive(Clone, Debug)]
enum CharRefKind {
    Special(char),
    Replacement(&'static str),
    Entity(String),
}

#[derive(Clone, Debug)]
enum LeafNode {
    LineBreak,
    Image {
        target: String,
        alt: Option<String>,
        width: Option<String>,
        height: Option<String>,
    },
    Reference {
        variant: RefVariant,
        target: String,
        roles: Vec<String>,
        window: Option<String>,
    },
    Anchor {
        id: String,
    },
    Callout {
        number: String,
    },
    IndexTerm {
        terms: Vec<String>,
        visible: bool,
    },
    Ui(UiMeta),
    Footnote {
        id: Option<String>,
        number: Option<String>,
        is_reference: bool,
    },
}

#[derive(Clone, Debug)]
enum UiMeta {
    Button(String),
    Keyboard(Vec<String>),
    Menu {
        menu: String,
        submenus: Vec<String>,
        item: Option<String>,
    },
}

// ─── The recording renderer ───────────────────────────────────────────────

/// A transparent decorator over [`HtmlSubstitutionRenderer`] that brackets each
/// recognized construct with recorder sentinels and captures its structural
/// metadata into a shared event log.
#[derive(Debug)]
struct RecordingRenderer {
    html: HtmlSubstitutionRenderer,
    events: Rc<RefCell<Vec<Event>>>,
}

impl RecordingRenderer {
    fn new() -> (Self, Rc<RefCell<Vec<Event>>>) {
        let events = Rc::new(RefCell::new(Vec::new()));

        let renderer = Self {
            html: HtmlSubstitutionRenderer {},
            events: events.clone(),
        };

        (renderer, events)
    }

    fn push(&self, event: Event) -> usize {
        let mut events = self.events.borrow_mut();
        events.push(event);
        events.len() - 1
    }

    /// Writes `MARK_OPEN index fragment MARK_CLOSE` into `dest`. `fragment` is
    /// the exact HTML the built-in renderer produced, so `dest` still contains
    /// every real byte and only inert sentinels are added.
    fn wrap(&self, index: usize, fragment: &str, dest: &mut String) {
        dest.push(MARK_OPEN);
        write_index(index, dest);
        dest.push_str(fragment);
        dest.push(MARK_CLOSE);
    }

    fn leaf(&self, node: LeafNode, fragment: &str, dest: &mut String) {
        let index = self.push(Event::Leaf(node));
        self.wrap(index, fragment, dest);
    }

    /// Recovers the fixed open/close wrapper a quote substitution emits, by
    /// rendering it once with [`SPLIT_PLACEHOLDER`] as the body and splitting.
    fn split_quote(
        &self,
        type_: QuoteType,
        scope: QuoteScope,
        attrlist: Option<Attrlist<'_>>,
        id: Option<String>,
    ) -> (String, String) {
        let mut probe = String::new();

        self.html.render_quoted_substitution(
            type_,
            scope,
            attrlist,
            id,
            &SPLIT_PLACEHOLDER.to_string(),
            &mut probe,
        );

        match probe.split_once(SPLIT_PLACEHOLDER) {
            Some((open, close)) => (open.to_string(), close.to_string()),

            // A body that does not survive the wrapper (should not happen for
            // any quote type) degrades to "whole fragment is the open".
            None => (probe, String::new()),
        }
    }
}

fn optional(value: Option<&str>) -> Option<String> {
    value.map(str::to_string)
}

impl InlineSubstitutionRenderer for RecordingRenderer {
    // `render_special_character` and `render_character_replacement` are
    // deliberately *not* overridden: their escaped output (`&lt;`, `&gt;`,
    // `&#8594;`, …) is re-matched by later steps (callouts on `&lt;N&gt;`, the
    // xref shorthand on `&lt;&lt;`, arrow replacements on `&lt;-`), so wrapping
    // it in sentinels would break that recognition. They fall through to the
    // built-in HTML renderer, and their `CharRef` nodes are recovered from the
    // resulting text runs (see `push_text`).

    fn render_quoted_substitution(
        &self,
        type_: QuoteType,
        scope: QuoteScope,
        attrlist: Option<Attrlist<'_>>,
        id: Option<String>,
        body: &str,
        dest: &mut String,
    ) {
        let roles: Vec<String> = attrlist
            .as_ref()
            .map(|a| a.roles().iter().map(|r| r.to_string()).collect())
            .unwrap_or_default();

        let (open, close) = self.split_quote(type_, scope, attrlist.clone(), id.clone());

        let node = match type_ {
            QuoteType::AsciiMath => ContainerNode::Stem {
                notation: StemNotation::AsciiMath,
            },

            QuoteType::LatexMath => ContainerNode::Stem {
                notation: StemNotation::LatexMath,
            },

            _ => ContainerNode::Styled {
                variant: style_variant(type_),
                form: span_form(scope),
                id: id.clone(),
                roles,
            },
        };

        let index = self.push(Event::Container {
            open: open.clone(),
            close: close.clone(),
            node,
        });

        // Re-render with the real body (which already carries the sentinels of
        // any nested constructs) so `dest` holds `open + body + close`, exactly
        // as the built-in renderer would, wrapped in this construct's markers.
        let mut fragment = String::new();
        self.html
            .render_quoted_substitution(type_, scope, attrlist, id, body, &mut fragment);

        self.wrap(index, &fragment, dest);
    }

    fn render_line_break(&self, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_line_break(&mut fragment);
        self.leaf(LeafNode::LineBreak, &fragment, dest);
    }

    fn render_image(&self, params: &ImageRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_image(params, &mut fragment);

        self.leaf(
            LeafNode::Image {
                target: params.target.to_string(),
                alt: Some(params.alt.clone()),
                width: optional(params.width),
                height: optional(params.height),
            },
            &fragment,
            dest,
        );
    }

    fn render_icon(&self, params: &IconRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_icon(params, &mut fragment);

        // The public vocabulary has no dedicated icon node yet; an icon projects
        // onto `Image` (its closest kin) for now.
        self.leaf(
            LeafNode::Image {
                target: params.target.to_string(),
                alt: Some(params.alt.clone()),
                width: None,
                height: None,
            },
            &fragment,
            dest,
        );
    }

    fn render_link(&self, params: &LinkRenderParams, dest: &mut String) {
        let mut roles: Vec<String> = params.extra_roles.iter().map(|r| r.to_string()).collect();
        roles.extend(params.attrlist.roles().iter().map(|r| r.to_string()));

        // A link's display text is inserted verbatim between a fixed open/close,
        // so – like a quote span – its wrapper is recovered by rendering once
        // with a placeholder body, and the real text becomes the container's
        // children (so `link:x[*bold*]` decomposes into a `Ref` over a `Styled`).
        let probe_params = LinkRenderParams {
            target: params.target.clone(),
            link_text: SPLIT_PLACEHOLDER.to_string(),
            extra_roles: params.extra_roles.clone(),
            window: params.window,
            attrlist: params.attrlist,
            parser: params.parser,
        };

        let mut probe = String::new();
        self.html.render_link(&probe_params, &mut probe);

        let (open, close) = match probe.split_once(SPLIT_PLACEHOLDER) {
            Some((open, close)) => (open.to_string(), close.to_string()),
            None => (probe, String::new()),
        };

        let index = self.push(Event::Container {
            open: open.clone(),
            close: close.clone(),
            node: ContainerNode::Reference {
                variant: RefVariant::Link,
                target: params.target.clone(),
                roles,
                window: optional(params.window),
            },
        });

        let mut fragment = String::new();
        self.html.render_link(params, &mut fragment);

        self.wrap(index, &fragment, dest);
    }

    fn render_anchor(&self, id: &str, reftext: Option<String>, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_anchor(id, reftext, &mut fragment);
        self.leaf(LeafNode::Anchor { id: id.to_string() }, &fragment, dest);
    }

    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_xref(params, &mut fragment);

        self.leaf(
            LeafNode::Reference {
                variant: RefVariant::Xref,
                target: params.target.to_string(),
                roles: params.roles.to_vec(),
                window: optional(params.window),
            },
            &fragment,
            dest,
        );
    }

    fn render_callout(&self, params: &CalloutRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_callout(params, &mut fragment);

        self.leaf(
            LeafNode::Callout {
                number: params.number.to_string(),
            },
            &fragment,
            dest,
        );
    }

    fn render_index_term(&self, params: &IndexTermRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_index_term(params, &mut fragment);

        // `render_index_term` only receives the visible primary term; the
        // concealed levels are not exposed here, so the term list is
        // best-effort in this bring-up oracle.
        let (terms, visible) = match params.visible_term {
            Some(term) => (vec![term.to_string()], true),
            None => (vec![], false),
        };

        self.leaf(LeafNode::IndexTerm { terms, visible }, &fragment, dest);
    }

    fn render_button(&self, text: &str, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_button(text, &mut fragment);
        self.leaf(
            LeafNode::Ui(UiMeta::Button(text.to_string())),
            &fragment,
            dest,
        );
    }

    fn render_keyboard(&self, keys: &[String], dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_keyboard(keys, &mut fragment);
        self.leaf(
            LeafNode::Ui(UiMeta::Keyboard(keys.to_vec())),
            &fragment,
            dest,
        );
    }

    fn render_menu(&self, params: &MenuRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_menu(params, &mut fragment);

        self.leaf(
            LeafNode::Ui(UiMeta::Menu {
                menu: params.menu.to_string(),
                submenus: params.submenus.to_vec(),
                item: optional(params.menuitem),
            }),
            &fragment,
            dest,
        );
    }

    fn render_footnote(&self, params: &FootnoteRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.html.render_footnote(params, &mut fragment);

        self.leaf(
            LeafNode::Footnote {
                id: optional(params.id),
                number: optional(params.index),
                is_reference: params.is_reference,
            },
            &fragment,
            dest,
        );
    }
}

fn style_variant(type_: QuoteType) -> StyleVariant {
    match type_ {
        QuoteType::Strong => StyleVariant::Strong,
        QuoteType::Emphasis => StyleVariant::Emphasis,
        QuoteType::Monospaced => StyleVariant::Code,
        QuoteType::Mark => StyleVariant::Mark,
        QuoteType::Superscript => StyleVariant::Superscript,
        QuoteType::Subscript => StyleVariant::Subscript,
        QuoteType::DoubleQuote => StyleVariant::DoubleQuote,
        QuoteType::SingleQuote => StyleVariant::SingleQuote,

        // AsciiMath/LatexMath are handled as STEM before this is reached;
        // `Unquoted` and any residue map to the unquoted span.
        _ => StyleVariant::Unquoted,
    }
}

fn span_form(scope: QuoteScope) -> SpanForm {
    match scope {
        QuoteScope::Constrained => SpanForm::Constrained,
        QuoteScope::Unconstrained => SpanForm::Unconstrained,
    }
}

// ─── The recorded tree ──────────────────────────────────────────────────────

/// The internal Phase 1 tree. Each node carries what the fold needs to
/// reconstruct its bytes, alongside the structural metadata that projects to a
/// public [`InlineNode`].
#[derive(Clone, Debug)]
enum Rec {
    Text(String),

    /// A character reference recovered from a text run. `html` is the exact
    /// entity the built-in renderer emitted (`&lt;`, `&#8594;`, …).
    CharRef {
        html: String,
        kind: CharRefKind,
    },
    Leaf {
        html: String,
        node: LeafNode,
    },
    Container {
        open: String,
        close: String,
        children: Vec<Rec>,
        node: ContainerNode,
    },
}

/// Pushes `text` onto `out`, splitting it into plain [`Rec::Text`] and
/// [`Rec::CharRef`] runs. Every `&` in the built-in renderer's output opens an
/// entity (a literal `&` is escaped to `&amp;`), so an ampersand-to-semicolon
/// span is always a character reference.
fn push_text(text: &str, out: &mut Vec<Rec>) {
    if text.is_empty() {
        return;
    }

    let mut plain = String::new();
    let mut rest = text;

    while let Some(amp) = rest.find('&') {
        plain.push_str(&rest[..amp]);
        let after = &rest[amp..];

        let Some(semi) = after.find(';') else {
            // No terminator: not an entity (should not occur). Keep the `&`.
            plain.push('&');
            rest = &after[1..];
            continue;
        };

        let entity = &after[..=semi];

        if !plain.is_empty() {
            out.push(Rec::Text(std::mem::take(&mut plain)));
        }

        out.push(Rec::CharRef {
            html: entity.to_string(),
            kind: classify_entity(entity),
        });

        rest = &after[semi + 1..];
    }

    plain.push_str(rest);

    if !plain.is_empty() {
        out.push(Rec::Text(plain));
    }
}

fn classify_entity(entity: &str) -> CharRefKind {
    match entity {
        "&lt;" => CharRefKind::Special('<'),
        "&gt;" => CharRefKind::Special('>'),
        "&amp;" => CharRefKind::Special('&'),

        "&#169;" => CharRefKind::Replacement("\u{a9}"),
        "&#174;" => CharRefKind::Replacement("\u{ae}"),
        "&#8482;" => CharRefKind::Replacement("\u{2122}"),
        "&#8201;" => CharRefKind::Replacement("\u{2009}"),
        "&#8212;" => CharRefKind::Replacement("\u{2014}"),
        "&#8203;" => CharRefKind::Replacement("\u{200b}"),
        "&#8230;" => CharRefKind::Replacement("\u{2026}"),
        "&#8592;" => CharRefKind::Replacement("\u{2190}"),
        "&#8656;" => CharRefKind::Replacement("\u{21d0}"),
        "&#8594;" => CharRefKind::Replacement("\u{2192}"),
        "&#8658;" => CharRefKind::Replacement("\u{21d2}"),
        "&#8217;" => CharRefKind::Replacement("\u{2019}"),
        "&#8216;" => CharRefKind::Replacement("\u{2018}"),

        // A numeric or named entity written by the author, carried through as
        // written.
        other => CharRefKind::Entity(other.to_string()),
    }
}

/// Parses a recorder-marked string into the recorded tree, resolving each
/// marker against `events`.
fn parse(chars: &[char], events: &[Event]) -> Vec<Rec> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;

    while let Some(&c) = chars.get(i) {
        if c != MARK_OPEN {
            text.push(c);
            i += 1;
            continue;
        }

        push_text(&std::mem::take(&mut text), &mut out);

        i += 1;

        // Read the PUA-encoded event index.
        let mut index = 0usize;
        while let Some(&d) = chars.get(i) {
            if !is_pua_digit(d) {
                break;
            }

            index = index * 10 + (d as u32 - PUA_DIGIT_BASE) as usize;
            i += 1;
        }

        // Find the matching close, honoring nesting.
        let start = i;
        let mut depth = 1;

        while let Some(&d) = chars.get(i) {
            match d {
                MARK_OPEN => depth += 1,

                MARK_CLOSE => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }

                _ => {}
            }

            i += 1;
        }

        let region = chars.get(start..i).unwrap_or(&[]);

        // Skip the matching close.
        i += 1;

        if let Some(event) = events.get(index) {
            out.push(build_rec(event, region, events));
        }
    }

    push_text(&text, &mut out);

    out
}

fn build_rec(event: &Event, region: &[char], events: &[Event]) -> Rec {
    match event {
        Event::Container { open, close, node } => {
            // `region` is `open + body + close`; the body carries the nested
            // markers. Slice off the known (marker-free) wrapper and recurse.
            let open_len = open.chars().count();
            let close_len = close.chars().count();
            let body_end = region.len().saturating_sub(close_len);

            let body = region.get(open_len..body_end).unwrap_or(&[]);

            Rec::Container {
                open: open.clone(),
                close: close.clone(),
                children: parse(body, events),
                node: node.clone(),
            }
        }

        Event::Leaf(node) => Rec::Leaf {
            html: strip_markers(region),
            node: node.clone(),
        },
    }
}

/// Folds the recorded tree back to HTML: text verbatim, a leaf as its captured
/// HTML, a container as open + folded children + close.
fn fold(recs: &[Rec]) -> String {
    let mut out = String::new();
    fold_into(recs, &mut out);
    out
}

fn fold_into(recs: &[Rec], out: &mut String) {
    for rec in recs {
        match rec {
            Rec::Text(text) => out.push_str(text),

            Rec::CharRef { html, .. } => out.push_str(html),

            Rec::Leaf { html, .. } => out.push_str(html),

            Rec::Container {
                open,
                close,
                children,
                ..
            } => {
                out.push_str(open);
                fold_into(children, out);
                out.push_str(close);
            }
        }
    }
}

/// The number of recorded constructs (non-text nodes) in the tree – i.e. the
/// number of [`Event`]s the tree consumed. Used to cross-check the tree against
/// the recorder.
fn construct_count(recs: &[Rec]) -> usize {
    recs.iter()
        .map(|rec| match rec {
            // Text and recovered character references are not recorded events.
            Rec::Text(_) | Rec::CharRef { .. } => 0,
            Rec::Leaf { .. } => 1,
            Rec::Container { children, .. } => 1 + construct_count(children),
        })
        .sum()
}

// ─── Projection to the public inline vocabulary ─────────────────────────────

/// Projects the recorded tree onto the public [`InlineNode`] vocabulary, giving
/// every node the same coarse `location` (Phase 1 carries no precise spans; see
/// design §4.4).
fn to_inline<'src>(recs: &[Rec], span: Span<'src>) -> Vec<InlineNode<'src>> {
    recs.iter().map(|rec| node_of(rec, span)).collect()
}

fn node_of<'src>(rec: &Rec, span: Span<'src>) -> InlineNode<'src> {
    match rec {
        Rec::Text(text) => InlineNode::Text {
            value: CowStr::from(text.clone()),
            location: span,
        },

        Rec::CharRef { kind, .. } => InlineNode::CharRef {
            value: match kind {
                CharRefKind::Special(ch) => CharRef::Special(*ch),
                CharRefKind::Replacement(value) => CharRef::Replacement(value),
                CharRefKind::Entity(name) => CharRef::Entity(CowStr::from(name.clone())),
            },
            location: span,
        },

        Rec::Leaf { node, .. } => leaf_node_of(node, span),

        Rec::Container { children, node, .. } => match node {
            ContainerNode::Styled {
                variant,
                form,
                id,
                roles,
            } => InlineNode::Styled(Styled {
                variant: *variant,
                form: *form,
                id: id.clone().map(CowStr::from),
                roles: roles.iter().cloned().map(CowStr::from).collect(),
                attrs: None,
                children: to_inline(children, span),
                location: span,
            }),

            ContainerNode::Reference {
                variant,
                target,
                roles,
                window,
            } => InlineNode::Ref(Ref {
                variant: *variant,
                target: CowStr::from(target.clone()),
                children: to_inline(children, span),
                roles: roles.iter().cloned().map(CowStr::from).collect(),
                window: window.clone().map(CowStr::from),
                resolved: None,
                location: span,
            }),

            ContainerNode::Stem { notation } => InlineNode::Stem(Stem {
                notation: *notation,
                value: CowStr::from(fold(children)),
                location: span,
            }),
        },
    }
}

fn leaf_node_of<'src>(node: &LeafNode, span: Span<'src>) -> InlineNode<'src> {
    match node {
        LeafNode::LineBreak => InlineNode::LineBreak { location: span },

        LeafNode::Image {
            target,
            alt,
            width,
            height,
        } => InlineNode::Image(Image {
            target: CowStr::from(target.clone()),
            alt: alt.clone().map(CowStr::from),
            width: width.clone().map(CowStr::from),
            height: height.clone().map(CowStr::from),
            attrs: None,
            location: span,
        }),

        LeafNode::Reference {
            variant,
            target,
            roles,
            window,
        } => InlineNode::Ref(Ref {
            variant: *variant,
            target: CowStr::from(target.clone()),
            children: vec![],
            roles: roles.iter().cloned().map(CowStr::from).collect(),
            window: window.clone().map(CowStr::from),
            resolved: None,
            location: span,
        }),

        LeafNode::Anchor { id } => InlineNode::Anchor(Anchor {
            id: CowStr::from(id.clone()),
            reftext: None,
            location: span,
        }),

        LeafNode::Callout { number } => InlineNode::Callout(Callout {
            number: CowStr::from(number.clone()),
            location: span,
        }),

        LeafNode::IndexTerm { terms, visible } => InlineNode::IndexTerm(IndexTerm {
            terms: terms.iter().cloned().map(CowStr::from).collect(),
            visible: *visible,
            location: span,
        }),

        LeafNode::Ui(ui) => InlineNode::Ui(Ui {
            kind: match ui {
                UiMeta::Button(text) => UiKind::Button(CowStr::from(text.clone())),

                UiMeta::Keyboard(keys) => {
                    UiKind::Keyboard(keys.iter().cloned().map(CowStr::from).collect())
                }

                UiMeta::Menu {
                    menu,
                    submenus,
                    item,
                } => UiKind::Menu {
                    menu: CowStr::from(menu.clone()),
                    submenus: submenus.iter().cloned().map(CowStr::from).collect(),
                    item: item.clone().map(CowStr::from),
                },
            },
            location: span,
        }),

        LeafNode::Footnote {
            id,
            number,
            is_reference,
        } => InlineNode::Footnote(Footnote {
            id: id.clone().map(CowStr::from),
            number: number.clone().map(CowStr::from),
            is_reference: *is_reference,
            children: vec![],
            location: span,
        }),
    }
}

// ─── The oracle ─────────────────────────────────────────────────────────────

/// The result of running the Phase 1 oracle on one source string.
struct Oracle<'src> {
    /// The authoritative `rendered()` output of the unmodified pipeline.
    golden: String,

    /// The recorder-marked `rendered()` string (real HTML + inert sentinels).
    marked: String,

    /// The recorded tree, projected onto the public vocabulary.
    tree: Vec<InlineNode<'src>>,

    /// The number of constructs the recorder captured.
    events: usize,

    /// The number of open-markers that reached the final string.
    markers: usize,

    /// The number of marked construct nodes in the recorded tree (recovered
    /// character references excluded, as they are not recorded events).
    constructs: usize,
}

/// Enables `:experimental:` so the UI macros (`kbd:`, `btn:`, `menu:`) are
/// recognized. Harmless for every other fixture.
fn experimental(parser: Parser) -> Parser {
    parser.with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere)
}

/// Runs `source` through the pipeline twice under `group`: once with the
/// built-in HTML renderer (the golden output) and once with the recording
/// renderer (from which the tree is built). Both use a default [`Parser`].
fn oracle<'src>(source: &'src str, group: &SubstitutionGroup) -> Oracle<'src> {
    // Golden pass.
    let golden_parser = experimental(Parser::default());
    let mut golden_content = Content::from(Span::new(source));
    group.apply(&mut golden_content, &golden_parser, None);
    let golden = golden_content.rendered_owned();

    // Recording pass.
    let (recorder, events) = RecordingRenderer::new();
    let recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let mut recording_content = Content::from(Span::new(source));
    group.apply(&mut recording_content, &recording_parser, None);
    let marked = recording_content.rendered_owned();

    let markers = marked.matches(MARK_OPEN).count();
    let chars: Vec<char> = marked.chars().collect();
    let events = events.borrow();
    let recs = parse(&chars, &events);

    Oracle {
        golden,
        marked,
        tree: to_inline(&recs, Span::new(source)),
        events: events.len(),
        markers,
        constructs: construct_count(&recs),
    }
}

/// Asserts the two Phase 1 invariants for `source` under `group`:
///
/// 1. stripping the sentinels reproduces the golden output byte-for-byte, and
/// 2. folding the recorded tree reproduces the golden output byte-for-byte,
///
/// and cross-checks that the tree consumed exactly the recorded events. Returns
/// the built tree for callers that also want to assert on structure.
fn check<'src>(source: &'src str, group: &SubstitutionGroup) -> Vec<InlineNode<'src>> {
    let result = oracle(source, group);

    let stripped = strip_markers(&result.marked.chars().collect::<Vec<_>>());
    assert_eq!(
        stripped, result.golden,
        "sentinel strip diverged from rendered() for {source:?}"
    );

    // Rebuild the tree independently of `oracle` so the fold is exercised on the
    // exact same source (the tree in `result` already reflects it, but folding
    // needs the `Rec` form).
    let (recorder, events) = RecordingRenderer::new();
    let recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let mut content = Content::from(Span::new(source));
    group.apply(&mut content, &recording_parser, None);
    let marked = content.rendered_owned();
    let chars: Vec<char> = marked.chars().collect();
    let recs = parse(&chars, &events.borrow());

    assert_eq!(
        fold(&recs),
        result.golden,
        "fold of the inline tree diverged from rendered() for {source:?}"
    );

    // Cross-check the tree against the recorder along the honest chain
    // `constructs <= markers <= events`:
    //
    // * `markers <= events` – some recorded events (formatting inside a footnote's
    //   text) are extracted out of the block, so their marker never reaches the
    //   block string.
    // * `constructs <= markers` – some markers (formatting inside a construct
    //   rendered as a single leaf, e.g. a formatted xref reftext) are absorbed into
    //   that leaf's HTML rather than surfacing as their own node.
    //
    // A tree with *more* constructs than the recorder saw would mean the builder
    // invented structure, which these bounds catch.
    assert!(
        result.markers <= result.events,
        "{} markers exceeded {} recorded events for {source:?}",
        result.markers,
        result.events,
    );

    assert!(
        result.constructs <= result.markers,
        "tree has {} constructs but the block string held {} markers for {source:?}",
        result.constructs,
        result.markers,
    );

    result.tree
}

fn check_normal(source: &str) -> Vec<InlineNode<'_>> {
    check(source, &SubstitutionGroup::Normal)
}

// ─── Byte-parity corpus ─────────────────────────────────────────────────────
//
// A differential harness over a broad set of inline fixtures. Each asserts the
// two Phase 1 invariants; together they exercise every intercepted construct.
// This is the "corpus-wide differential harness" the design (§5.3) calls for,
// built from inline sources because the crate keeps no `.adoc` fixture files.

/// Inline fixtures covering the `normal` substitution group.
const NORMAL_CORPUS: &[&str] = &[
    // Plain text and the empty case.
    "",
    "plain text with no constructs",
    "text with trailing spaces   and   runs",
    // Special characters.
    "a < b && c > d",
    "1 < 2 & 3 > 0",
    "<>&",
    // Quotes: constrained.
    "One *word* is strong.",
    "An _emphasized_ phrase.",
    "Some `monospaced` text.",
    "A #highlighted# span.",
    "H~2~O and E = mc^2^.",
    "A bold *phrase of text* here.",
    // Quotes: unconstrained.
    "Bold c**hara**cter**s** in a word.",
    "un__frac__tured emphasis",
    "a##b##c",
    // Quotes: nested.
    "*a _b_ c*",
    "*bold _and italic_ mix*",
    "_*strong inside emphasis*_",
    // Quotes with roles / id shorthand.
    "[.myrole]#roled text#",
    "[#anchor]#anchored#",
    "[.a.b]#multi role#",
    // Quotes mixed with special characters (interleaving).
    "*a < b* and _c > d_",
    "code `x < y && z` here",
    // Smart quotes.
    "\"`double`\" and '`single`' quotes",
    // Character replacements.
    "(C) (R) (TM)",
    "An em -- dash and an ellipsis...",
    "arrows -> => <- <=",
    "Sam's apostrophe",
    "named &amp; numeric &#8217; entities",
    // Attribute references expand (no macro).
    "plain {backend} attribute",
    // Macros: links.
    "See https://example.org for details.",
    "A link:https://example.org[example] link.",
    "link:https://example.org[with *bold* text]",
    "mailto:a@b.com[email me]",
    // Macros: images / icons.
    "an image:photo.png[Alt Text] inline",
    "image:pic.png[Scaled,200,100]",
    // Macros: cross-references (unresolved fallback).
    "see <<target>> for more",
    "see <<target,the target>> now",
    "xref:other.adoc#frag[Other] doc",
    // Macros: footnotes.
    "A claim.footnote:[the evidence]",
    "Named.footnote:disc[a discussion] then footnote:disc[].",
    // Macros: UI.
    "Press kbd:[Ctrl+T] now.",
    "Press kbd:[Ctrl,Shift,N] now.",
    "Click btn:[Save] please.",
    "Choose menu:File[Save As > PDF].",
    // Macros: index terms.
    "A flow ((visible term)) here.",
    "A concealed (((primary,secondary))) term.",
    // Macros: anchors.
    "[[the-anchor]]Anchored paragraph.",
    // Line breaks (post-replacement).
    "first line +\nsecond line",
    // Combined stress.
    "*bold* _em_ `code` (C) https://x.y[link] <<ref>> image:i.png[i]",
    "A mix of {backend}, *bold < text*, and a footnote:[with `code`].",
    // Escapes (formatting suppressed).
    r"a \*not bold* b",
    r"an \_not emphasized_ here",
    r"literal \`backtick\` text",
    // Deeper / adjacent nesting.
    "*a _b `c` d_ e*",
    "*one* *two* *three*",
    "_a_ and _b_ and _c_",
    "`code with *stars* inside`",
    // Unconstrained with adjacent text.
    "pre**mid**post and a__b__c",
    "x^sup^y and p~sub~q",
    // Multiple roles and an id together.
    "[.role1.role2#the-id]#decorated#",
    "[#only-id]#text#",
    // Monospace with special chars.
    "`a < b > c & d`",
    // Entities and replacements interleaved.
    "(C) then (R) then (TM) end",
    "First... then -- and -> arrows <-",
    "a &amp; b and &#8482; and &copy;",
    // Attribute reference that is undefined (kept verbatim) and defined.
    "an {undefined-attr} reference",
    // Links with attributes and roles.
    "link:https://example.org[Example,role=external,window=_blank]",
    "https://example.org[Example^]",
    "a https://example.org[] bare-macro link",
    // Autolinks in text.
    "visit https://example.org/path?q=1 now",
    // Images with roles / alt containing spaces.
    "image:a.png[An image with spaces,role=thumb]",
    "before image:b.svg[Vector] after",
    // Several xrefs (unresolved fallback).
    "<<a>> and <<b>> and <<c,C text>>",
    "xref:sec[with *bold* reftext]",
    // Index terms of each kind.
    "a ((flow term)) and (((c1, c2, c3))) end",
    "indexterm:[primary, secondary]",
    "indexterm2:[shown]",
    // Anchors in different positions.
    "[[mid-anchor]] after the anchor",
    "text before anchor:named[Ref Text] and after",
    // Passthroughs (should survive verbatim as-is).
    "a +++<b>raw</b>+++ passthrough",
    "inline pass:[<i>x</i>] macro",
    "math $$a < b$$ here",
    "a +literal *stars*+ b",
    // Hard breaks across multiple lines.
    "line one +\nline two +\nline three",
    // STEM inline (asciimath / latexmath).
    "stem:[a < b] expression",
    // Empty-ish and whitespace.
    "   ",
    "a\nb\nc",
];

/// Inline fixtures for the `verbatim` group (special characters + callouts).
const VERBATIM_CORPUS: &[&str] = &[
    "plain verbatim",
    "verbatim < & > chars",
    "line of code <1>",
    "tag <.> auto callout",
];

#[test]
fn normal_corpus_folds_byte_for_byte() {
    for source in NORMAL_CORPUS {
        check(source, &SubstitutionGroup::Normal);
    }
}

// ─── Document-level differential ────────────────────────────────────────────
//
// The block-level harness above drives `SubstitutionGroup::apply` on a bare
// `Content`, which cannot resolve cross-references (that needs a document
// catalog). This harness parses whole documents – once normally, once with the
// recorder installed – and asserts that folding each simple block's recorded
// tree reproduces its `rendered()` string. It reaches resolved xrefs, list
// items, section bodies, and the interactions between blocks.

fn simple_blocks_map<T>(
    doc: &crate::Document<'_>,
    mut f: impl FnMut(&crate::blocks::SimpleBlock<'_>) -> T,
) -> Vec<T> {
    fn walk<T>(
        block: &crate::blocks::Block<'_>,
        f: &mut impl FnMut(&crate::blocks::SimpleBlock<'_>) -> T,
        out: &mut Vec<T>,
    ) {
        if let crate::blocks::Block::Simple(simple) = block {
            out.push(f(simple));
        }

        for child in block.child_blocks() {
            walk(child, f, out);
        }
    }

    let mut out = vec![];

    for block in doc.child_blocks() {
        walk(block, &mut f, &mut out);
    }

    out
}

/// Parses `source` twice – normally and with the recorder – and asserts that
/// folding every simple block's recorded tree reproduces its `rendered()`
/// output byte-for-byte, including any resolved cross-references.
fn check_document(source: &str) {
    let mut golden_parser = experimental(Parser::default());
    let golden_doc = golden_parser.parse(source);
    let golden = simple_blocks_map(&golden_doc, |simple| {
        simple.content().rendered().to_string()
    });

    let (recorder, events) = RecordingRenderer::new();
    let mut recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let recording_doc = recording_parser.parse(source);
    let events = events.borrow();

    let folded = simple_blocks_map(&recording_doc, |simple| {
        let marked: Vec<char> = simple.content().rendered().chars().collect();
        fold(&parse(&marked, &events))
    });

    assert_eq!(
        folded, golden,
        "document fold diverged from rendered() for {source:?}"
    );
}

/// Whole-document fixtures, including resolved cross-references (which the
/// block-level harness cannot reach).
const DOCUMENT_CORPUS: &[&str] = &[
    // Resolved cross-reference via an explicit anchor.
    "[[tgt]]The target paragraph.\n\nSee <<tgt>> for details.",
    "[[tgt]]Target.\n\nSee <<tgt,the target>> now.",
    // Several references, one unresolved.
    "[[a]]First.\n\n[[b]]Second.\n\nSee <<a>>, <<b>>, and <<missing>>.",
    // Multiple paragraphs with formatting.
    "First *para* here.\n\nSecond _para_ with `code` and (C).",
    // Footnotes across paragraphs.
    "Body text.footnote:[the first note]\n\nMore text.footnote:[the second note]",
    // A list with inline formatting per item.
    "* item *one*\n* item _two_ with `code`\n* item with a < b",
    // A section whose body references an explicitly-anchored target.
    "== A Section\n\n[[para]]Some text.\n\nBack to <<para>>.",
    // Image and link together in a paragraph.
    "See image:x.png[X] and also https://example.org[the site].",
    // Ordered list with entities and a quote.
    ". one -- two\n. three *four*",
    // A description list term and description.
    "Term:: a *bold* description with a < b",
    // Cross-reference to an auto-generated section ID (derived from a plain
    // title): a stress test that the recorder does not perturb ID generation.
    "== Introduction\n\nText.\n\n== Details\n\nSee <<_introduction>> above.",
    // A simple table whose cells carry inline formatting.
    "|===\n| *bold* cell | a < b\n| plain | (C)\n|===",
];

#[test]
fn document_corpus_folds_byte_for_byte() {
    for source in DOCUMENT_CORPUS {
        check_document(source);
    }
}

/// A known limitation of the Strategy A oracle at document scope: when a
/// section title carries inline formatting, its *rendered* text (now bearing
/// recorder sentinels) feeds the cross-reference catalog as the reference's
/// display text, and a later `<<auto-id>>` to that section resolves differently
/// in the recorded parse than in the golden one. This is the "double render can
/// drift" property the design calls out (§4.1); it is retired by the
/// single-pass builder in Phase 4, which never re-renders. Plain-title sections
/// and explicitly-anchored targets (exercised above) are unaffected. Recorded
/// as an ignored test so the boundary is explicit rather than hidden.
#[test]
#[ignore = "Strategy A perturbs xref reftext derived from a formatted section title; fixed in Phase 4"]
fn formatted_section_title_xref_is_a_known_strategy_a_gap() {
    check_document("== The *Bold* Heading\n\nBody.\n\nSee <<_the_bold_heading>>.");
}

#[test]
fn verbatim_corpus_folds_byte_for_byte() {
    for source in VERBATIM_CORPUS {
        check(source, &SubstitutionGroup::Verbatim);
    }
}

// ─── Structural assertions ──────────────────────────────────────────────────
//
// Net-new guarantees the golden HTML string cannot express: node kinds and
// nesting. These confirm the tree is a real structural artifact, not merely a
// byte-parity shim.

#[test]
fn plain_text_is_a_single_borrowed_text_node() {
    let tree = check_normal("plain text");

    assert_eq!(tree.len(), 1);
    assert!(matches!(&tree[0], InlineNode::Text { value, .. } if value.as_ref() == "plain text"));
}

#[test]
fn special_character_splits_into_text_and_charref() {
    let tree = check_normal("a < b");

    // "a " → Text, "<" → CharRef, " b" → Text.
    assert_eq!(tree.len(), 3);
    assert!(matches!(&tree[0], InlineNode::Text { .. }));
    assert!(matches!(
        &tree[1],
        InlineNode::CharRef {
            value: CharRef::Special('<'),
            ..
        }
    ));
    assert!(matches!(&tree[2], InlineNode::Text { .. }));
}

#[test]
fn strong_wraps_a_text_child() {
    let tree = check_normal("*bold*");

    assert_eq!(tree.len(), 1);

    let InlineNode::Styled(styled) = &tree[0] else {
        panic!("expected a styled node, got {:?}", tree[0]);
    };

    assert_eq!(styled.variant, StyleVariant::Strong);
    assert_eq!(styled.form, SpanForm::Constrained);
    assert_eq!(styled.children.len(), 1);
    assert!(
        matches!(&styled.children[0], InlineNode::Text { value, .. } if value.as_ref() == "bold")
    );
}

#[test]
fn nested_formatting_is_a_tree() {
    // `*a _b_ c*` → Strong[ Text("a "), Emphasis[ Text("b") ], Text(" c") ].
    let tree = check_normal("*a _b_ c*");

    let InlineNode::Styled(strong) = &tree[0] else {
        panic!("expected strong, got {:?}", tree[0]);
    };

    assert_eq!(strong.variant, StyleVariant::Strong);
    assert_eq!(strong.children.len(), 3);

    let InlineNode::Styled(em) = &strong.children[1] else {
        panic!("expected nested emphasis, got {:?}", strong.children[1]);
    };

    assert_eq!(em.variant, StyleVariant::Emphasis);
    assert_eq!(em.children.len(), 1);
    assert!(matches!(&em.children[0], InlineNode::Text { value, .. } if value.as_ref() == "b"));
}

#[test]
fn unconstrained_form_is_recorded() {
    let tree = check_normal("a**b**c");

    let InlineNode::Styled(strong) = &tree[1] else {
        panic!("expected strong at index 1, got {:?}", tree);
    };

    assert_eq!(strong.variant, StyleVariant::Strong);
    assert_eq!(strong.form, SpanForm::Unconstrained);
}

#[test]
fn role_shorthand_is_captured_on_the_span() {
    let tree = check_normal("[.myrole]#text#");

    let InlineNode::Styled(styled) = &tree[0] else {
        panic!("expected styled, got {:?}", tree[0]);
    };

    assert!(
        styled.roles.iter().any(|r| r.as_ref() == "myrole"),
        "roles were {:?}",
        styled.roles
    );
}

#[test]
fn character_replacement_is_a_charref() {
    let tree = check_normal("(C)");

    assert_eq!(tree.len(), 1);
    assert!(matches!(
        &tree[0],
        InlineNode::CharRef {
            value: CharRef::Replacement(_),
            ..
        }
    ));
}

#[test]
fn inline_image_is_an_image_node() {
    let tree = check_normal("image:photo.png[Alt Text]");

    let image = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Image(image) => Some(image),
            _ => None,
        })
        .expect("expected an image node");

    assert_eq!(image.target.as_ref(), "photo.png");
    assert_eq!(image.alt.as_deref(), Some("Alt Text"));
}

#[test]
fn unresolved_xref_is_a_ref_node() {
    let tree = check_normal("see <<target>> here");

    let reference = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Ref(reference) => Some(reference),
            _ => None,
        })
        .expect("expected a ref node");

    assert_eq!(reference.variant, RefVariant::Xref);
    assert_eq!(reference.target.as_ref(), "target");
}

#[test]
fn footnote_marker_is_a_footnote_node() {
    let tree = check_normal("A claim.footnote:[evidence]");

    let footnote = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Footnote(footnote) => Some(footnote),
            _ => None,
        })
        .expect("expected a footnote node");

    assert!(!footnote.is_reference);
    assert_eq!(footnote.number.as_deref(), Some("1"));
}

#[test]
fn keyboard_macro_is_a_ui_node() {
    let tree = check_normal("Press kbd:[Ctrl+T].");

    let ui = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Ui(ui) => Some(ui),
            _ => None,
        })
        .expect("expected a ui node");

    let UiKind::Keyboard(keys) = &ui.kind else {
        panic!("expected a keyboard macro, got {:?}", ui.kind);
    };

    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].as_ref(), "Ctrl");
    assert_eq!(keys[1].as_ref(), "T");
}

#[test]
fn line_break_is_a_linebreak_node() {
    let tree = check("first +\nsecond", &SubstitutionGroup::Normal);

    assert!(
        tree.iter()
            .any(|n| matches!(n, InlineNode::LineBreak { .. })),
        "expected a line-break node in {tree:?}"
    );
}

#[test]
fn callout_in_verbatim_is_a_callout_node() {
    let tree = check("some code <1>", &SubstitutionGroup::Verbatim);

    assert!(
        tree.iter().any(|n| matches!(n, InlineNode::Callout(_))),
        "expected a callout node in {tree:?}"
    );
}
