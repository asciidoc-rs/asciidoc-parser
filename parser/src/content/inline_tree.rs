//! Builds an inline AST by **recording the string pipeline's own renderer
//! calls** — the retired "Strategy A" construction, kept as **test-only oracle
//! machinery**.
//!
//! This module was the production tree source through Phase 2 and Phase 4's
//! additive increments: when tree building was enabled on the [`Parser`] — an
//! opt-in that has since retired along with it —
//! `SubstitutionGroup::apply` re-ran the pipeline through the
//! [`RecordingRenderer`] and parsed the recorded markers into each
//! [`Content`]'s tree. The Phase 4 single-pass builder
//! ([`inline_builder`](crate::content::inline_builder)) has since replaced it
//! as `Content`'s tree source (the first half of the design's step 6
//! cutover), so this module is now compiled only for tests, where it remains
//! the independent construction the differential harness
//! ([`inline_recorder`]) and the structural cross-check
//! (`tests::inline_builder_recorder_parity`) compare the builder against.
//!
//! # Strategy A, unchanged
//!
//! The construction strategy is still "Strategy A" (design §4.1): the existing
//! substitution pipeline is re-run with a **transparent marker-recording
//! decorator** ([`RecordingRenderer`]) wrapped around the parser's own inline
//! renderer. The decorator writes the exact bytes the wrapped renderer would,
//! and additionally brackets each recognized construct with inert
//! Private-Use-Area sentinels. Two facts follow directly (and are asserted by
//! the Phase 1 corpus):
//!
//! 1. Stripping the sentinels reproduces the wrapped renderer's output
//!    byte-for-byte (the *no-perturbation* invariant).
//! 2. Parsing the sentinel structure recovers a tree of [`InlineNode`]s, and
//!    folding that tree reconstructs the same bytes.
//!
//! Because the decorator wraps whatever renderer the [`Parser`] carries, the
//! fold reproduces *that* renderer's bytes, not a hard-coded HTML backend.
//!
//! # Why a second pass
//!
//! Making the tree the recognition sink directly — so there is no second pass
//! and no markers — is "Strategy B", the Phase 4 single-pass builder that has
//! now replaced this module in production. Here the tree is built by a
//! **second, counter-safe substitution pass**: the caller clones the
//! [`Parser`] *before* the authoritative pass advances any document counter,
//! so the recording pass numbers footnotes, callouts, and counters exactly as
//! the authoritative pass did, then discards the clone. The authoritative
//! rendered string is never produced by the recorder, so the pass is purely
//! additive and cannot regress output — which is also what makes it a clean
//! *independent* construction for the cross-check to compare against.
//!
//! [`inline_recorder`]: ../../tests/inline_recorder.rs
//! [`Content`]: crate::content::Content
//! [`Parser`]: crate::Parser

// Some node metadata is captured but not yet read by every consumer while the
// tree is not yet canonical; the tree carries the full structure the public API
// (Phase 3) and the single-pass builder (Phase 4) will consume.
#![allow(dead_code)]

use std::{cell::RefCell, rc::Rc};

use crate::{
    Span,
    attributes::Attrlist,
    inlines::{
        Anchor, Callout, CalloutGuard, CharRef, Footnote, Image, IndexTerm, InlineNode, Ref,
        RefVariant, SpanForm, Stem, StemNotation, StyleVariant, Styled, Ui, UiKind,
    },
    parser::{
        CalloutGuard as ParserCalloutGuard, CalloutRenderParams, FootnoteRenderParams,
        IconRenderParams, ImageRenderParams, IndexTermRenderParams, InlineSubstitutionRenderer,
        LinkRenderParams, MenuRenderParams, QuoteScope, QuoteType, XrefRenderParams,
    },
    strings::CowStr,
};

// ─── Sentinels ────────────────────────────────────────────────────────────
//
// All in the Unicode Private Use Area, chosen to be inert to every step's
// regexes, exactly as the production pipeline's xref (`\u{E000}`) and
// passthrough (`\u{96}`) sentinels already are. Digits are encoded as PUA
// characters too, so a recorded marker injects no ASCII word characters that a
// later step could key off of.
//
// Unlike the three production sentinel systems, these never reach shipped
// output: they are stripped (or folded away) the instant the recording pass
// finishes, before the tree is stored on the `Content`.

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

/// Reports whether `c` is a codepoint the recorder reserves for its own framing
/// (a marker, a marker index digit, or the split placeholder).
pub(crate) fn is_reserved_sentinel(c: char) -> bool {
    is_marker(c) || c == SPLIT_PLACEHOLDER
}

/// Removes every recorder sentinel from `chars`, leaving the real bytes.
fn strip_markers(chars: &[char]) -> String {
    chars.iter().copied().filter(|c| !is_marker(*c)).collect()
}

// ─── Recorded events ────────────────────────────────────────────────────────

/// The metadata captured for one recognized construct, looked up by the index
/// encoded in its [`MARK_OPEN`] marker.
#[derive(Clone, Debug)]
pub(crate) enum Event {
    /// A parent construct (a [quote substitution] or a link). `open`/`close`
    /// are the fixed wrapper bytes the wrapped renderer emits around the body.
    ///
    /// [quote substitution]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
    Container {
        open: String,
        close: String,
        node: ContainerNode,
    },

    /// A leaf construct. Its rendered bytes are recovered from the marked
    /// string, so only the structural metadata is stored here.
    Leaf(LeafNode),
}

#[derive(Clone, Debug)]
pub(crate) enum ContainerNode {
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
/// entities the wrapped renderer emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CharRefKind {
    Special(char),
    Replacement(&'static str),
    Entity(String),
}

#[derive(Clone, Debug)]
pub(crate) enum LeafNode {
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
        guard: CalloutGuardMeta,
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

/// Owned counterpart of [`crate::parser::CalloutGuard`], captured from
/// [`CalloutRenderParams`] at recording time (this module stores owned
/// strings throughout, matching Strategy A's "owned strings" limitation).
#[derive(Clone, Debug)]
pub(crate) enum CalloutGuardMeta {
    LineComment(String),
    Xml,
}

#[derive(Clone, Debug)]
pub(crate) enum UiMeta {
    Button(String),
    Keyboard(Vec<String>),
    Menu {
        menu: String,
        submenus: Vec<String>,
        item: Option<String>,
    },
}

// ─── The recording renderer ───────────────────────────────────────────────

/// A transparent decorator over the parser's own inline renderer that brackets
/// each recognized construct with recorder sentinels and captures its
/// structural metadata into a shared event log.
///
/// Every method delegates to the wrapped `inner` renderer, so the bytes are
/// whatever `inner` would have produced; the decorator only adds inert
/// sentinels (and, for the structural constructs, records an [`Event`]).
#[derive(Debug)]
pub(crate) struct RecordingRenderer {
    inner: Rc<dyn InlineSubstitutionRenderer>,
    events: Rc<RefCell<Vec<Event>>>,
}

impl RecordingRenderer {
    /// Wraps `inner`, returning the decorator and a handle to the shared event
    /// log it fills in (the log is read back by [`build_inline_tree`]).
    pub(crate) fn new(
        inner: Rc<dyn InlineSubstitutionRenderer>,
    ) -> (Self, Rc<RefCell<Vec<Event>>>) {
        let events = Rc::new(RefCell::new(Vec::new()));

        let renderer = Self {
            inner,
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
    /// the exact output the wrapped renderer produced, so `dest` still contains
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

        self.inner.render_quoted_substitution(
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
    // `render_special_character` and `render_character_replacement` delegate to
    // `inner` *without* bracketing: their escaped output (`&lt;`, `&gt;`,
    // `&#8594;`, …) is re-matched by later steps (callouts on `&lt;N&gt;`, the
    // xref shorthand on `&lt;&lt;`, arrow replacements on `&lt;-`), so wrapping
    // it in sentinels would break that recognition. Their `CharRef` nodes are
    // recovered from the resulting text runs instead (see `push_text`).

    fn render_special_character(&self, type_: crate::parser::SpecialCharacter, dest: &mut String) {
        self.inner.render_special_character(type_, dest);
    }

    fn render_character_replacement(
        &self,
        type_: crate::parser::CharacterReplacementType,
        dest: &mut String,
    ) {
        self.inner.render_character_replacement(type_, dest);
    }

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
        // as the wrapped renderer would, bracketed in this construct's markers.
        let mut fragment = String::new();
        self.inner
            .render_quoted_substitution(type_, scope, attrlist, id, body, &mut fragment);

        self.wrap(index, &fragment, dest);
    }

    fn render_line_break(&self, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_line_break(&mut fragment);
        self.leaf(LeafNode::LineBreak, &fragment, dest);
    }

    fn render_image(&self, params: &ImageRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_image(params, &mut fragment);

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
        self.inner.render_icon(params, &mut fragment);

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
        // so — like a quote span — its wrapper is recovered by rendering once
        // with a placeholder body, and the real text becomes the container's
        // children (so `link:x[*bold*]` decomposes into a `Ref` over a `Styled`).
        let probe_params = LinkRenderParams {
            target: params.target.clone(),
            link_text: SPLIT_PLACEHOLDER.to_string(),
            extra_roles: params.extra_roles.clone(),
            window: params.window,
            attrlist: params.attrlist,
            context: params.context,
        };

        let mut probe = String::new();
        self.inner.render_link(&probe_params, &mut probe);

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
        self.inner.render_link(params, &mut fragment);

        self.wrap(index, &fragment, dest);
    }

    fn render_anchor(&self, id: &str, reftext: Option<String>, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_anchor(id, reftext, &mut fragment);
        self.leaf(LeafNode::Anchor { id: id.to_string() }, &fragment, dest);
    }

    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_xref(params, &mut fragment);

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
        self.inner.render_callout(params, &mut fragment);

        let guard = match params.guard {
            ParserCalloutGuard::LineComment(prefix) => {
                CalloutGuardMeta::LineComment(prefix.to_string())
            }
            ParserCalloutGuard::Xml => CalloutGuardMeta::Xml,
        };

        self.leaf(
            LeafNode::Callout {
                number: params.number.to_string(),
                guard,
            },
            &fragment,
            dest,
        );
    }

    fn render_index_term(&self, params: &IndexTermRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_index_term(params, &mut fragment);

        // `render_index_term` only receives the visible primary term; the
        // concealed levels are not exposed here, so the term list is
        // best-effort while the tree is not yet the recognition sink.
        let (terms, visible) = match params.visible_term {
            Some(term) => (vec![term.to_string()], true),
            None => (vec![], false),
        };

        self.leaf(LeafNode::IndexTerm { terms, visible }, &fragment, dest);
    }

    fn render_button(&self, text: &str, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_button(text, &mut fragment);
        self.leaf(
            LeafNode::Ui(UiMeta::Button(text.to_string())),
            &fragment,
            dest,
        );
    }

    fn render_keyboard(&self, keys: &[String], dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_keyboard(keys, &mut fragment);
        self.leaf(
            LeafNode::Ui(UiMeta::Keyboard(keys.to_vec())),
            &fragment,
            dest,
        );
    }

    fn render_menu(&self, params: &MenuRenderParams, dest: &mut String) {
        let mut fragment = String::new();
        self.inner.render_menu(params, &mut fragment);

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
        self.inner.render_footnote(params, &mut fragment);

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

/// The intermediate tree recovered from the marked string. Each node carries
/// what the fold needs to reconstruct its bytes, alongside the structural
/// metadata that projects to a public [`InlineNode`].
#[derive(Clone, Debug)]
enum Rec {
    Text(String),

    /// A character reference recovered from a text run. `html` is the exact
    /// entity the wrapped renderer emitted (`&lt;`, `&#8594;`, …).
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
/// [`Rec::CharRef`] runs. Every `&` in the wrapped renderer's output opens an
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

pub(crate) fn classify_entity(entity: &str) -> CharRefKind {
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

/// Parses a recorder-marked string into the recovered tree, resolving each
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

/// Folds the recovered tree back to output bytes: text verbatim, a leaf as its
/// captured bytes, a container as open + folded children + close.
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

// ─── Projection to the public inline vocabulary ─────────────────────────────

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
                // The Strategy-A recorder recovers a link from its rendered
                // `<a …>` markup, which the three spellings share, so it has
                // no way to tell them apart; the single-pass builder sets this
                // honestly (as with an image's `is_icon`).
                link_form: None,
                target: CowStr::from(target.clone()),
                children: to_inline(children, span),
                roles: roles.iter().cloned().map(CowStr::from).collect(),
                window: window.clone().map(CowStr::from),
                resolved: None,
                derived: None,
                xrefstyle: None,
                attrs: None,
                location: span,
            }),

            ContainerNode::Stem { notation } => {
                let mut value = String::new();
                fold_into(children, &mut value);

                InlineNode::Stem(Stem {
                    notation: *notation,
                    value: CowStr::from(value),
                    location: span,
                })
            }
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
            // The Strategy-A recorder does not distinguish `icon:` from
            // `image:` (both fold through its own recorded markers), so it
            // leaves this `false`; the single-pass builder sets it honestly.
            // Likewise the restored ranges: the recorder's target is the
            // rendered string's own (already-restored) bytes, with no record
            // of which came from a masked construct.
            is_icon: false,
            target: CowStr::from(target.clone()),
            restored_target_ranges: vec![],
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
            // Not distinguishable from the rendered markup — see the container
            // arm above.
            link_form: None,
            target: CowStr::from(target.clone()),
            children: vec![],
            roles: roles.iter().cloned().map(CowStr::from).collect(),
            window: window.clone().map(CowStr::from),
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: None,
            location: span,
        }),

        LeafNode::Anchor { id } => InlineNode::Anchor(Anchor {
            id: CowStr::from(id.clone()),
            reftext: None,
            is_bibliography: false,
            location: span,
        }),

        LeafNode::Callout { number, guard } => InlineNode::Callout(Callout {
            number: CowStr::from(number.clone()),
            guard: match guard {
                CalloutGuardMeta::LineComment(prefix) => {
                    CalloutGuard::LineComment(CowStr::from(prefix.clone()))
                }
                CalloutGuardMeta::Xml => CalloutGuard::Xml,
            },
            location: span,
        }),

        LeafNode::IndexTerm { terms, visible } => InlineNode::IndexTerm(IndexTerm {
            terms: terms.iter().cloned().map(CowStr::from).collect(),
            // The recorder recovers a shown term from the *rendered* string, so
            // it always has one as text; only the single-pass builder reaches
            // the structural spelling (see `IndexTerm::children`).
            children: vec![],
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

// ─── Public (crate) entry points ────────────────────────────────────────────

/// Builds the inline tree from a recorder-marked string and its event log,
/// giving every node the coarse `location` span (Phase 1/2 carry no precise
/// per-node spans; see design §4.4). `marked` is the string a
/// [`RecordingRenderer`] produced; `events` is the log it filled in.
pub(crate) fn build_inline_tree<'src>(
    marked: &str,
    events: &[Event],
    location: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let chars: Vec<char> = marked.chars().collect();
    let recs = parse(&chars, events);
    to_inline(&recs, location)
}

/// Populates the child subtree of every *defining* [`Footnote`] node in
/// `nodes`, from the recorder-marked footnote texts the recording pass
/// registered.
///
/// A footnote's text is extracted out of the flow of the block during the
/// macros substitution step — only its marker is left behind — so it never
/// reaches the block's marked string and cannot be recovered by
/// [`build_inline_tree`] alone. `texts` is the marked text of each footnote the
/// recording pass defined, in registration order (as
/// `Parser::footnote_texts_from` reports it); each parses against the *same*
/// `events` log, since the recorder brackets a footnote's constructs there too.
///
/// The defining footnote nodes of a block, visited in document order, line up
/// one-to-one with the footnotes that block's pipeline registered, so node *i*
/// takes text *i*. A *reference* occurrence (`footnote:id[]`) defines nothing
/// and keeps the empty subtree its node type documents.
///
/// [`Footnote`]: crate::inlines::Footnote
pub(crate) fn attach_footnote_subtrees<'src>(
    nodes: &mut [InlineNode<'src>],
    texts: &[String],
    events: &[Event],
    location: Span<'src>,
) {
    if texts.is_empty() {
        return;
    }

    let mut next = 0;
    attach_footnote_texts(nodes, texts, events, location, &mut next);

    // Every footnote the pipeline defined must have found its node. A mismatch
    // means the recording pass and the authoritative pass enumerated footnotes
    // differently, which would misplace a subtree; catch that in debug/test
    // builds rather than storing a wrong tree.
    debug_assert_eq!(
        next,
        texts.len(),
        "inline tree footnote count diverged from the footnotes the pipeline defined",
    );
}

/// Walks `nodes` in document order, giving each defining [`Footnote`] node the
/// subtree parsed from its marked text, and advancing `next` past it.
///
/// [`Footnote`]: crate::inlines::Footnote
fn attach_footnote_texts<'src>(
    nodes: &mut [InlineNode<'src>],
    texts: &[String],
    events: &[Event],
    location: Span<'src>,
    next: &mut usize,
) {
    for node in nodes {
        match node {
            InlineNode::Footnote(footnote) => {
                // A bare reference to an earlier footnote defines nothing, so it
                // consumes no text and keeps its empty subtree.
                if footnote.is_reference {
                    continue;
                }

                if let Some(text) = texts.get(*next) {
                    let chars: Vec<char> = text.chars().collect();
                    footnote.children = to_inline(&parse(&chars, events), location);
                }

                *next += 1;
            }

            InlineNode::Styled(styled) => {
                attach_footnote_texts(&mut styled.children, texts, events, location, next);
            }

            InlineNode::Ref(reference) => {
                attach_footnote_texts(&mut reference.children, texts, events, location, next);
            }

            _ => {}
        }
    }
}

/// Folds a recorder-marked string back to output bytes. Used by the
/// differential tests to assert the fold reproduces the authoritative rendered
/// string.
pub(crate) fn fold_marked(marked: &str, events: &[Event]) -> String {
    let chars: Vec<char> = marked.chars().collect();
    let recs = parse(&chars, events);
    let mut out = String::new();
    fold_into(&recs, &mut out);
    out
}

/// The number of open-markers in a recorder-marked string — i.e. the number of
/// recorded constructs that reached the string. Used by the differential tests
/// to cross-check the recovered tree against the recorder.
pub(crate) fn open_marker_count(marked: &str) -> usize {
    marked.matches(MARK_OPEN).count()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::rc::Rc;

    use super::*;
    use crate::{
        Parser, Span,
        attributes::{Attrlist, AttrlistContext},
        parser::{LinkRenderParams, QuoteScope, QuoteType},
    };

    /// A renderer that discards the body it is handed, so the recorder's
    /// placeholder probe never survives — exercising the defensive
    /// "the wrapper did not contain the placeholder body" fallbacks in
    /// [`RecordingRenderer::split_quote`] and
    /// [`RecordingRenderer::render_link`]. A real renderer always echoes the
    /// body, so these paths are otherwise unreachable.
    #[derive(Debug)]
    struct DroppingRenderer;

    impl InlineSubstitutionRenderer for DroppingRenderer {
        fn render_quoted_substitution(
            &self,
            _type_: QuoteType,
            _scope: QuoteScope,
            _attrlist: Option<Attrlist<'_>>,
            _id: Option<String>,
            _body: &str,
            dest: &mut String,
        ) {
            dest.push_str("<q/>");
        }

        fn render_link(&self, _params: &LinkRenderParams, dest: &mut String) {
            dest.push_str("<a/>");
        }
    }

    #[test]
    fn push_text_keeps_an_unterminated_ampersand() {
        // The wrapped renderer always emits terminated entities, but the
        // ampersand-without-`;` guard keeps a stray `&` as plain text rather
        // than mis-reading it as an entity.
        let mut out = Vec::new();
        push_text("a & b", &mut out);

        let mut folded = String::new();
        fold_into(&out, &mut folded);
        assert_eq!(folded, "a & b");

        // No character reference was manufactured from the bare ampersand.
        assert!(out.iter().all(|rec| !matches!(rec, Rec::CharRef { .. })));
    }

    #[test]
    fn quoted_substitution_falls_back_when_the_body_is_dropped() {
        let (recorder, events) = RecordingRenderer::new(Rc::new(DroppingRenderer));

        let mut dest = String::new();
        recorder.render_quoted_substitution(
            QuoteType::Strong,
            QuoteScope::Constrained,
            None,
            None,
            "body",
            &mut dest,
        );

        // A container event was still recorded, and the fallback did not panic.
        assert_eq!(events.borrow().len(), 1);
        assert!(dest.contains("<q/>"));
    }

    #[test]
    fn link_falls_back_when_the_text_is_dropped() {
        let (recorder, events) = RecordingRenderer::new(Rc::new(DroppingRenderer));

        let parser = Parser::default();
        let attrlist = Attrlist::parse(Span::new(""), &parser, AttrlistContext::Inline)
            .item
            .item;

        let params = LinkRenderParams {
            target: "https://example.org".to_string(),
            link_text: "text".to_string(),
            extra_roles: vec![],
            window: None,
            attrlist: &attrlist,
            context: &parser.render_context(),
        };

        let mut dest = String::new();
        recorder.render_link(&params, &mut dest);

        assert_eq!(events.borrow().len(), 1);
        assert!(dest.contains("<a/>"));
    }
}
