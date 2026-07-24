//! Semantic inline AST for issue #892.
//!
//! The parser's inline substitution pipeline is a faithful port of
//! Asciidoctor's ordered, *destructive* string rewrite: each
//! [`SubstitutionStep`](crate::content::SubstitutionStep) runs a
//! `Regex::replace_all` over the whole rendered string, so the only artifact
//! that survives is a flat HTML string. Issue #892 asks for access to the
//! *semantic* structure underneath (bold, links, cross-references, footnotes,
//! …) so downstream tools (Zola theming, coverage, structural diff) don't have
//! to regex the rendered HTML.
//!
//! # Approach
//!
//! We do **not** rebuild the substitution engine or thread source offsets
//! through every step (the invasive option rejected in issue #564). Instead we
//! reuse the seam the crate already has: every piece of final output flows
//! through an [`InlineSubstitutionRenderer`], which is told *semantically* what
//! it is rendering. That renderer is swappable on the [`Parser`]
//! (`parser.renderer`), so we run the ordinary pipeline a second time with a
//! recording renderer ([`InlineAstRecorder`]) that, instead of writing HTML,
//! writes lightweight Private-Use-Area **markers** bracketing each semantic
//! element (generalizing exactly what the crate already does for deferred
//! cross-references and section-title footnotes). Because every step rewrites
//! the same shared string, the markers nest naturally; a final single pass
//! ([`parse_marked`]) turns the marker-laced string into a tree.
//!
//! The capture is additive and touches none of the existing rendering path, so
//! [`Content::rendered`](crate::content::Content::rendered) is byte-for-byte
//! unchanged. It is opt-in
//! ([`Parser::with_inline_ast_capture`](crate::Parser::with_inline_ast_capture)),
//! since it runs substitution a second time per content block.
//!
//! # Correct numbering via pre-substitution capture
//!
//! The AST pass runs on a *clone* of the parser taken **before** the canonical
//! pass mutates it. Every stateful side effect of substitution – counters,
//! footnote numbering, the reference catalog – lives in `RefCell`/`Cell` fields
//! that the parser's derived `Clone` deep-copies, so the clone (a) cannot
//! perturb the real document and (b) starts from the identical state, and thus
//! reproduces the *same* counter and footnote numbers the rendered output uses.
//! See [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup) for the
//! re-entrancy guard that stops nested substitutions (passthrough restore,
//! table cells) from recursively capturing.
//!
//! # Cross-reference resolution
//!
//! Capture happens at parse time, before references are resolved, so an
//! [`InlineNode::Xref`] is first recorded with only its source target and text.
//! The document's later resolution pass calls
//! [`resolve_xref_nodes`] (from
//! [`Content::resolve_references`](crate::content::Content)), filling each
//! xref's `href` – so after parsing, block-content cross-references expose
//! their resolved destination.
//!
//! # Known limitations
//!
//! * **No per-node source spans, and node text is owned (not borrowed from the
//!   source).** Both are consequences of the marker approach, not oversights.
//!   The tree is parsed from the *rendered* buffer (each substitution step
//!   rebuilds `Content::rendered` as a fresh owned `String`), which no longer
//!   maps back to source offsets – the very correlation issue #564 declined to
//!   thread through the pipeline – and whose slices cannot outlive the pass, so
//!   they must be copied. Consumers that need to locate content in source can
//!   use the block-granular
//!   [`Content::original`](crate::content::Content::original). True per-node
//!   spans (and zero-copy borrowing) would require making the AST the primary
//!   artifact and deriving HTML from it – a larger refactor tracked separately.
//! * Deeply nested quote formatting mirrors Asciidoctor's regex-ordering
//!   quirks: whichever quote sub runs first is the outer marked node.
//! * Cross-references inside footnote text or section titles resolve for the
//!   rendered output but are not yet reflected back into their captured AST
//!   nodes (only block-content xrefs are).

use std::{cell::RefCell, rc::Rc};

use crate::{
    attributes::Attrlist,
    content::{Content, SubstitutionGroup},
    parser::{
        CalloutRenderParams, CharacterReplacementType, FootnoteRenderParams, IconRenderParams,
        ImageRenderParams, IndexTermRenderParams, InlineSubstitutionRenderer, LinkRenderParams,
        MenuRenderParams, Parser, QuoteScope, QuoteType, ReferenceResolver, ResolutionContext,
        SpecialCharacter, XrefRenderParams,
    },
};

/// A node in the semantic inline AST exposed by
/// [`Content::inline_nodes`](crate::content::Content::inline_nodes).
///
/// This is an owned tree: node text is copied out of the substitution buffer
/// rather than borrowed from the source. (The rendered buffer the tree is
/// parsed from is rebuilt as it goes and does not map back to source offsets,
/// so the nodes carry no per-node source spans; use
/// [`Content::original`](crate::content::Content::original) for block-granular
/// location.)
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum InlineNode {
    /// A run of literal text between formatting elements. Special characters
    /// and character replacements are folded into the surrounding text (as
    /// their rendered form), matching what a reader sees.
    Text(String),

    /// A styled span (`*strong*`, `_emphasis_`, `` `monospace` ``, `#mark#`,
    /// `^super^`, `~sub~`, smart quotes, an unstyled `[.role]#…#` span, or
    /// inline math). Its `children` are the parsed inner content, so
    /// nesting is preserved.
    Styled {
        /// Which style this span applies.
        style: InlineStyle,

        /// An explicit id (`[#id]#…#`), if any.
        id: Option<String>,

        /// Roles (`[.role]#…#`) applied to the span.
        roles: Vec<String>,

        /// The parsed inner content.
        children: Vec<InlineNode>,
    },

    /// A hyperlink (`link:`, `https://…[…]`, `mailto:`). The display text is
    /// captured as a string because the pipeline hands the renderer already
    /// rendered link text (it is not re-flowed through the recorder).
    Link {
        /// The link target (URL).
        target: String,

        /// The display text.
        text: String,

        /// Roles applied to the link.
        roles: Vec<String>,

        /// Target window (`window=_blank`), if any.
        window: Option<String>,
    },

    /// A cross-reference (`<<id>>`, `xref:id[…]`).
    Xref {
        /// The raw cross-reference target.
        target: String,

        /// Explicit link text, if supplied.
        text: Option<String>,

        /// Roles supplied on the `xref:` macro.
        roles: Vec<String>,

        /// The resolved (or target-derived) hyperlink destination, filled in by
        /// the document's reference-resolution pass. `None` while unresolved
        /// (e.g. the standalone [`SubstitutionGroup::inline_nodes`] API, which
        /// has no document context, or a target that could not be resolved).
        ///
        /// [`SubstitutionGroup::inline_nodes`]: crate::content::SubstitutionGroup::inline_nodes
        href: Option<String>,
    },

    /// An inline image (`image:target[alt]`).
    Image {
        /// The image target.
        target: String,

        /// The alt text (explicit or defaulted).
        alt: String,
    },

    /// An inline footnote marker (`footnote:[…]`, `footnote:id[…]`).
    Footnote {
        /// The footnote's id, if it was given one.
        id: Option<String>,

        /// The footnote's number. This matches the rendered output: capture
        /// runs from the same document-order state, so the counter is shared.
        index: Option<String>,

        /// `true` when this occurrence references an existing footnote.
        is_reference: bool,

        /// The footnote text for an unresolved reference (empty otherwise).
        text: String,
    },

    /// A hard line break (from the post-replacement substitution).
    LineBreak,

    /// A passthrough span (`+…+`, `+++…+++`, `pass:[…]`) with no quote
    /// formatting of its own. Its content is emitted verbatim in the rendered
    /// output; here it is preserved as its own node so consumers can see it was
    /// a passthrough rather than ordinary text.
    Passthrough(String),

    /// An inline anchor (`[[id]]`, `anchor:id[]`).
    Anchor {
        /// The anchor id.
        id: String,
    },

    /// A catch-all leaf for inline macros not yet modeled in detail (keyboard,
    /// button, menu, icon, index term, callout). `kind` names the macro; `text`
    /// is a best-effort display string.
    Macro {
        /// A short kind label (e.g. `"kbd"`, `"menu"`, `"icon"`).
        kind: &'static str,

        /// A best-effort display string for the macro.
        text: String,
    },
}

/// The style applied by an [`InlineNode::Styled`] span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InlineStyle {
    /// `*strong*` (bold).
    Strong,

    /// `_emphasis_` (italic).
    Emphasis,

    /// `` `monospace` `` (code).
    Monospace,

    /// `#mark#` (highlight).
    Mark,

    /// `^superscript^`.
    Superscript,

    /// `~subscript~`.
    Subscript,

    /// Smart double quotes (`"`…`"`).
    DoubleQuote,

    /// Smart single quotes (`'`…`'`).
    SingleQuote,

    /// An unstyled span, e.g. `[.role]#…#` carrying only roles/id.
    Unquoted,

    /// Inline AsciiMath.
    AsciiMath,

    /// Inline LaTeX math.
    LatexMath,
}

impl InlineNode {
    /// Returns the flattened text content of this node and its descendants,
    /// ignoring formatting. Handy for coverage/diff consumers that want the
    /// words without the markup.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        self.write_text_content(&mut out);
        out
    }

    fn write_text_content(&self, out: &mut String) {
        match self {
            InlineNode::Text(t) => out.push_str(t),

            InlineNode::Styled { children, .. } => {
                for child in children {
                    child.write_text_content(out);
                }
            }

            InlineNode::Link { text, .. } => out.push_str(text),

            InlineNode::Xref { text, target, .. } => {
                out.push_str(text.as_deref().unwrap_or(target));
            }

            InlineNode::Image { alt, .. } => out.push_str(alt),

            InlineNode::Passthrough(text) => out.push_str(text),

            InlineNode::Footnote { .. } | InlineNode::LineBreak | InlineNode::Anchor { .. } => {}

            InlineNode::Macro { text, .. } => out.push_str(text),
        }
    }
}

// ---------------------------------------------------------------------------
// Marker scheme
// ---------------------------------------------------------------------------

/// Private-Use-Area codepoints bracketing a recorded node in the marker stream.
///
/// These are distinct from the cross-reference/footnote sentinels in
/// [`content`](crate::content) (`\u{E000}`–`\u{E003}`) so the two mechanisms
/// compose without collision. Like those, they cannot occur in user text and
/// are inert to the substitution regexes.
///
/// A node is emitted as `START <index> MID <children> END`; a leaf emits an
/// empty body (`START <index> MID END`).
const AST_START: char = '\u{E010}';
const AST_MID: char = '\u{E011}';
const AST_END: char = '\u{E012}';

/// A recorded semantic event, indexed by the marker embedded in the stream.
/// This mirrors [`InlineNode`] minus the child structure, which is recovered
/// from the marker nesting at parse time.
#[derive(Clone, Debug)]
enum Raw {
    Styled {
        style: InlineStyle,
        id: Option<String>,
        roles: Vec<String>,
    },
    Link {
        target: String,
        text: String,
        roles: Vec<String>,
        window: Option<String>,
    },
    Xref {
        target: String,
        text: Option<String>,
        roles: Vec<String>,
        href: Option<String>,
    },
    Image {
        target: String,
        alt: String,
    },
    Footnote {
        id: Option<String>,
        index: Option<String>,
        is_reference: bool,
        text: String,
    },
    LineBreak,
    Passthrough(String),
    Anchor {
        id: String,
    },
    Macro {
        kind: &'static str,
        text: String,
    },
}

/// An [`InlineSubstitutionRenderer`] that records the semantic structure of the
/// inline content instead of (only) producing HTML.
///
/// For each element the pipeline renders, it appends a [`Raw`] record and emits
/// marker sentinels around the element's body. Text-like substitutions (special
/// characters, character replacements) are delegated to the wrapped `inner`
/// renderer so the recovered [`InlineNode::Text`] carries exactly the text a
/// reader would see.
#[derive(Debug)]
pub(crate) struct InlineAstRecorder {
    inner: Rc<dyn InlineSubstitutionRenderer>,
    records: RefCell<Vec<Raw>>,
}

impl InlineAstRecorder {
    pub(crate) fn new(inner: Rc<dyn InlineSubstitutionRenderer>) -> Self {
        Self {
            inner,
            records: RefCell::new(vec![]),
        }
    }

    /// Records `raw` and returns its index.
    fn push(&self, raw: Raw) -> usize {
        let mut records = self.records.borrow_mut();
        records.push(raw);
        records.len() - 1
    }

    /// Emits a leaf node: `START <index> MID END`.
    fn emit_leaf(&self, raw: Raw, dest: &mut String) {
        let index = self.push(raw);
        emit_start(index, dest);
        dest.push(AST_END);
    }

    /// Emits the opening `START <index> MID` of a container; the caller appends
    /// the body and then [`emit_container_end`](Self::emit_container_end).
    fn emit_container_start(&self, raw: Raw, dest: &mut String) -> usize {
        let index = self.push(raw);
        emit_start(index, dest);
        index
    }
}

fn emit_start(index: usize, dest: &mut String) {
    dest.push(AST_START);
    dest.push_str(&index.to_string());
    dest.push(AST_MID);
}

impl InlineSubstitutionRenderer for InlineAstRecorder {
    fn render_special_character(&self, type_: SpecialCharacter, dest: &mut String) {
        // Fold into surrounding text as the reader-visible form.
        self.inner.render_special_character(type_, dest);
    }

    fn render_character_replacement(&self, type_: CharacterReplacementType, dest: &mut String) {
        self.inner.render_character_replacement(type_, dest);
    }

    fn render_quoted_substitution(
        &self,
        type_: QuoteType,
        _scope: QuoteScope,
        attrlist: Option<Attrlist<'_>>,
        id: Option<String>,
        body: &str,
        dest: &mut String,
    ) {
        let style = match type_ {
            QuoteType::Strong => InlineStyle::Strong,
            QuoteType::Emphasis => InlineStyle::Emphasis,
            QuoteType::Monospaced => InlineStyle::Monospace,
            QuoteType::Mark => InlineStyle::Mark,
            QuoteType::Superscript => InlineStyle::Superscript,
            QuoteType::Subscript => InlineStyle::Subscript,
            QuoteType::DoubleQuote => InlineStyle::DoubleQuote,
            QuoteType::SingleQuote => InlineStyle::SingleQuote,
            QuoteType::Unquoted => InlineStyle::Unquoted,
            QuoteType::AsciiMath => InlineStyle::AsciiMath,
            QuoteType::LatexMath => InlineStyle::LatexMath,
        };

        let id = id.or_else(|| attrlist.as_ref().and_then(|a| a.id().map(str::to_owned)));

        let roles = attrlist
            .as_ref()
            .map(|a| a.roles().into_iter().map(str::to_owned).collect())
            .unwrap_or_default();

        self.emit_container_start(Raw::Styled { style, id, roles }, dest);
        // `body` was itself produced through this recorder, so it already
        // carries the markers for any nested elements.
        dest.push_str(body);
        dest.push(AST_END);
    }

    fn render_line_break(&self, dest: &mut String) {
        self.emit_leaf(Raw::LineBreak, dest);
    }

    fn render_passthrough(&self, text: &str, dest: &mut String) {
        self.emit_leaf(Raw::Passthrough(text.to_owned()), dest);
    }

    fn render_image(&self, params: &ImageRenderParams, dest: &mut String) {
        self.emit_leaf(
            Raw::Image {
                target: params.target.to_owned(),
                alt: params.alt.clone(),
            },
            dest,
        );
    }

    fn image_uri(
        &self,
        target_image_path: &str,
        parser: &Parser,
        asset_dir_key: Option<&str>,
    ) -> String {
        self.inner
            .image_uri(target_image_path, parser, asset_dir_key)
    }

    fn render_icon(&self, params: &IconRenderParams, dest: &mut String) {
        self.emit_leaf(
            Raw::Macro {
                kind: "icon",
                text: params.target.to_owned(),
            },
            dest,
        );
    }

    fn render_link(&self, params: &LinkRenderParams, dest: &mut String) {
        let mut roles: Vec<String> = params
            .attrlist
            .roles()
            .into_iter()
            .map(str::to_owned)
            .collect();
        roles.extend(params.extra_roles.iter().map(|r| (*r).to_owned()));

        self.emit_leaf(
            Raw::Link {
                target: params.target.clone(),
                text: params.link_text.clone(),
                roles,
                window: params.window.map(str::to_owned),
            },
            dest,
        );
    }

    fn render_anchor(&self, id: &str, _reftext: Option<String>, dest: &mut String) {
        self.emit_leaf(Raw::Anchor { id: id.to_owned() }, dest);
    }

    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
        // At capture time a same-document reference is usually still unresolved
        // (resolution is a later document pass); a target that names a document
        // already carries its derived destination. Either fills in later via
        // `resolve_xref_nodes`.
        let href = params
            .resolved
            .map(|r| r.href.clone())
            .or_else(|| params.derived.map(|d| d.href.clone()));

        self.emit_leaf(
            Raw::Xref {
                target: params.target.to_owned(),
                text: params.provided_text.map(str::to_owned),
                roles: params.roles.to_vec(),
                href,
            },
            dest,
        );
    }

    fn render_callout(&self, params: &CalloutRenderParams, dest: &mut String) {
        self.emit_leaf(
            Raw::Macro {
                kind: "callout",
                text: params.number.to_owned(),
            },
            dest,
        );
    }

    fn render_index_term(&self, params: &IndexTermRenderParams, dest: &mut String) {
        // A concealed index term produces no visible output and no node.
        if let Some(term) = params.visible_term {
            self.emit_leaf(
                Raw::Macro {
                    kind: "index-term",
                    text: term.to_owned(),
                },
                dest,
            );
        }
    }

    fn render_button(&self, text: &str, dest: &mut String) {
        self.emit_leaf(
            Raw::Macro {
                kind: "button",
                text: text.to_owned(),
            },
            dest,
        );
    }

    fn render_keyboard(&self, keys: &[String], dest: &mut String) {
        self.emit_leaf(
            Raw::Macro {
                kind: "kbd",
                text: keys.join("+"),
            },
            dest,
        );
    }

    fn render_menu(&self, params: &MenuRenderParams, dest: &mut String) {
        let mut text = String::from(params.menu);
        for submenu in params.submenus {
            text.push_str(" > ");
            text.push_str(submenu);
        }

        if let Some(item) = params.menuitem {
            text.push_str(" > ");
            text.push_str(item);
        }

        self.emit_leaf(Raw::Macro { kind: "menu", text }, dest);
    }

    fn render_footnote(&self, params: &FootnoteRenderParams, dest: &mut String) {
        self.emit_leaf(
            Raw::Footnote {
                id: params.id.map(str::to_owned),
                index: params.index.map(str::to_owned),
                is_reference: params.is_reference,
                text: params.text.to_owned(),
            },
            dest,
        );
    }
}

// ---------------------------------------------------------------------------
// Marker-stream parser
// ---------------------------------------------------------------------------

/// Parses a marker-laced string produced by [`InlineAstRecorder`] into a tree.
fn parse_marked(s: &str, records: &[Raw]) -> Vec<InlineNode> {
    let (nodes, rest) = parse_seq(s, records);

    // The stream is always balanced (every START pairs with an END), so the top
    // level consumes everything. A non-empty remainder means a stray END, which
    // cannot arise from the recorder; surface it in debug builds rather than
    // silently dropping content.
    debug_assert!(rest.is_empty(), "unbalanced inline-AST marker stream");

    nodes
}

/// Parses a sequence of nodes until an unmatched [`AST_END`] (or end of input).
/// Returns the nodes and the remainder of the string *after* the closing
/// [`AST_END`] (empty at the top level).
fn parse_seq<'a>(mut s: &'a str, records: &[Raw]) -> (Vec<InlineNode>, &'a str) {
    let mut out: Vec<InlineNode> = vec![];
    let mut buf = String::new();

    loop {
        let Some(pos) = s.find([AST_START, AST_END]) else {
            buf.push_str(s);
            flush_text(&mut buf, &mut out);
            return (out, "");
        };

        buf.push_str(&s[..pos]);

        // `find` matched one of our single-char sentinels, so this is safe.
        let control = s[pos..].chars().next().unwrap_or(AST_END);
        let after = &s[pos + control.len_utf8()..];

        if control == AST_END {
            flush_text(&mut buf, &mut out);
            return (out, after);
        }

        // A START marker: `AST_START <index> AST_MID <body> AST_END`.
        flush_text(&mut buf, &mut out);

        let Some(mid) = after.find(AST_MID) else {
            // Malformed; emit nothing further and stop.
            debug_assert!(false, "inline-AST START without MID");
            return (out, "");
        };

        let index: usize = after[..mid].parse().unwrap_or(usize::MAX);
        let body = &after[mid + AST_MID.len_utf8()..];

        let (children, rest) = parse_seq(body, records);

        if let Some(node) = records.get(index).map(|raw| build_node(raw, children)) {
            out.push(node);
        } else {
            debug_assert!(false, "inline-AST index {index} out of range");
        }

        s = rest;
    }
}

/// Pushes accumulated literal text as a [`InlineNode::Text`] (if non-empty) and
/// clears the buffer.
fn flush_text(buf: &mut String, out: &mut Vec<InlineNode>) {
    if !buf.is_empty() {
        out.push(InlineNode::Text(std::mem::take(buf)));
    }
}

/// Builds an [`InlineNode`] from a [`Raw`] record and its parsed children.
/// Leaf records ignore `children` (which is empty for them by construction).
fn build_node(raw: &Raw, children: Vec<InlineNode>) -> InlineNode {
    match raw {
        Raw::Styled { style, id, roles } => InlineNode::Styled {
            style: *style,
            id: id.clone(),
            roles: roles.clone(),
            children,
        },

        Raw::Link {
            target,
            text,
            roles,
            window,
        } => InlineNode::Link {
            target: target.clone(),
            text: text.clone(),
            roles: roles.clone(),
            window: window.clone(),
        },

        Raw::Xref {
            target,
            text,
            roles,
            href,
        } => InlineNode::Xref {
            target: target.clone(),
            text: text.clone(),
            roles: roles.clone(),
            href: href.clone(),
        },

        Raw::Image { target, alt } => InlineNode::Image {
            target: target.clone(),
            alt: alt.clone(),
        },

        Raw::Footnote {
            id,
            index,
            is_reference,
            text,
        } => InlineNode::Footnote {
            id: id.clone(),
            index: index.clone(),
            is_reference: *is_reference,
            text: text.clone(),
        },

        Raw::LineBreak => InlineNode::LineBreak,

        Raw::Passthrough(text) => InlineNode::Passthrough(text.clone()),

        Raw::Anchor { id } => InlineNode::Anchor { id: id.clone() },

        Raw::Macro { kind, text } => InlineNode::Macro {
            kind,
            text: text.clone(),
        },
    }
}

// ---------------------------------------------------------------------------
// Pipeline entry point
// ---------------------------------------------------------------------------

/// Runs `group`'s substitution pipeline over a copy of `content` with a
/// recording renderer and returns the resulting inline AST.
///
/// `content` should be in its pre-substitution state (its `rendered` still
/// holds the filtered source text), so the AST pass sees exactly what the
/// canonical pass will. This clones `parser` so the AST pass – which re-runs
/// stateful substitutions – does not perturb the caller's parser state; because
/// all such state lives in `RefCell`/`Cell` fields that `Parser`'s derived
/// `Clone` deep- copies, the clone reproduces identical counter and footnote
/// numbering. The clone's renderer is wrapped so text-like substitutions render
/// identically to the real output, and its re-entrancy guard is set so this
/// pass never recursively captures.
pub(crate) fn capture_inline_nodes(
    content: &Content<'_>,
    group: &SubstitutionGroup,
    parser: &Parser,
    attrlist: Option<&Attrlist>,
) -> Vec<InlineNode> {
    let recorder = Rc::new(InlineAstRecorder::new(parser.renderer.clone()));

    let mut ast_parser = parser.clone();
    ast_parser.renderer = recorder.clone();
    ast_parser.inline_ast_capturing.set(true);

    let mut ast_content = content.clone();
    group.apply(&mut ast_content, &ast_parser, attrlist);

    let records = recorder.records.borrow();
    parse_marked(ast_content.rendered_str(), &records)
}

/// Fills in the resolved hyperlink destination on every [`InlineNode::Xref`] in
/// `nodes` (recursing into styled spans), using the document's `resolver`.
///
/// This is the AST counterpart of the deferred cross-reference resolution that
/// rebuilds [`Content::rendered`]; each cross-reference is resolved by its own
/// target, so it is robust to ordering. A target the resolver cannot place
/// keeps whatever destination was derived at capture time (a document target),
/// or stays `None`.
pub(crate) fn resolve_xref_nodes(nodes: &mut [InlineNode], resolver: &dyn ReferenceResolver) {
    for node in nodes {
        match node {
            InlineNode::Xref {
                target, text, href, ..
            } => {
                if let Some(resolved) = resolver.resolve(&ResolutionContext {
                    target,
                    provided_text: text.as_deref(),
                    derived: None,
                }) {
                    *href = Some(resolved.href);

                    if text.is_none() {
                        *text = resolved.text;
                    }
                }
            }

            InlineNode::Styled { children, .. } => resolve_xref_nodes(children, resolver),

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{InlineNode, InlineStyle};
    use crate::{Span, content::SubstitutionGroup, tests::prelude::*};

    /// Convenience: build the inline AST for `src` under the Normal group.
    fn nodes(src: &str) -> Vec<InlineNode> {
        let p = Parser::default();
        SubstitutionGroup::Normal.inline_nodes(Span::new(src), &p, None)
    }

    #[test]
    fn plain_text_is_one_text_node() {
        assert_eq!(
            nodes("just words"),
            vec![InlineNode::Text("just words".into())]
        );
    }

    #[test]
    fn special_characters_fold_into_text() {
        // `<`, `>`, `&` are rendered into the text, not separate nodes.
        assert_eq!(
            nodes("a < b & c"),
            vec![InlineNode::Text("a &lt; b &amp; c".into())]
        );
    }

    #[test]
    fn strong_word() {
        assert_eq!(
            nodes("One *word* here"),
            vec![
                InlineNode::Text("One ".into()),
                InlineNode::Styled {
                    style: InlineStyle::Strong,
                    id: None,
                    roles: vec![],
                    children: vec![InlineNode::Text("word".into())],
                },
                InlineNode::Text(" here".into()),
            ]
        );
    }

    #[test]
    fn emphasis_and_monospace() {
        assert_eq!(
            nodes("_em_ and `code`"),
            vec![
                InlineNode::Styled {
                    style: InlineStyle::Emphasis,
                    id: None,
                    roles: vec![],
                    children: vec![InlineNode::Text("em".into())],
                },
                InlineNode::Text(" and ".into()),
                InlineNode::Styled {
                    style: InlineStyle::Monospace,
                    id: None,
                    roles: vec![],
                    children: vec![InlineNode::Text("code".into())],
                },
            ]
        );
    }

    #[test]
    fn marked_span_with_id_and_role() {
        let n = nodes("[#hi.warn]#alert#");
        assert_eq!(
            n,
            vec![InlineNode::Styled {
                style: InlineStyle::Unquoted,
                id: Some("hi".into()),
                roles: vec!["warn".into()],
                children: vec![InlineNode::Text("alert".into())],
            }]
        );
    }

    #[test]
    fn nested_strong_containing_emphasis() {
        // Whichever quote sub runs first is the outer node; assert the structure
        // rather than a specific nesting order, and check the flattened text.
        let n = nodes("*_both_*");
        assert_eq!(n.len(), 1);

        let outer = n.first().unwrap();
        assert_eq!(outer.text_content(), "both");

        // The outer span is a styled span containing a single nested styled span
        // (no stray text).
        assert!(
            matches!(
                outer,
                InlineNode::Styled { children, .. }
                    if matches!(children.as_slice(), [InlineNode::Styled { .. }])
            ),
            "expected a styled span nesting a styled span, got {outer:?}"
        );
    }

    #[test]
    fn link_captures_target_and_text() {
        assert_eq!(
            nodes("see https://example.com[the site] now"),
            vec![
                InlineNode::Text("see ".into()),
                InlineNode::Link {
                    target: "https://example.com".into(),
                    text: "the site".into(),
                    roles: vec![],
                    window: None,
                },
                InlineNode::Text(" now".into()),
            ]
        );
    }

    #[test]
    fn inline_image() {
        let n = nodes("image:sunset.png[Sunset]");
        assert_eq!(
            n,
            vec![InlineNode::Image {
                target: "sunset.png".into(),
                alt: "Sunset".into(),
            }]
        );
    }

    #[test]
    fn hard_line_break_node() {
        // A trailing ` +` is a hard line break (post-replacement substitution).
        let n = nodes("line one +\nline two");
        assert!(
            n.iter().any(|node| matches!(node, InlineNode::LineBreak)),
            "expected a LineBreak node, got {n:?}"
        );
    }

    #[test]
    fn xref_captures_target() {
        let n = nodes("see <<intro>> please");
        let xref = n
            .iter()
            .find(|node| matches!(node, InlineNode::Xref { .. }))
            .unwrap();

        assert_eq!(
            xref,
            &InlineNode::Xref {
                target: "intro".into(),
                text: None,
                roles: vec![],
                href: None,
            }
        );
    }

    #[test]
    fn xref_with_provided_text() {
        let n = nodes("xref:intro[the intro]");
        assert_eq!(
            n,
            vec![InlineNode::Xref {
                target: "intro".into(),
                text: Some("the intro".into()),
                roles: vec![],
                href: None,
            }]
        );
    }

    #[test]
    fn passthrough_is_its_own_node() {
        // A triple-plus passthrough emits its content verbatim and is preserved
        // as a Passthrough node rather than folded into the text.
        assert_eq!(
            nodes("a +++<b>raw</b>+++ z"),
            vec![
                InlineNode::Text("a ".into()),
                InlineNode::Passthrough("<b>raw</b>".into()),
                InlineNode::Text(" z".into()),
            ]
        );
    }

    #[test]
    fn footnote_captures_text_structure() {
        let n = nodes("text.footnote:[a note]");
        let has_footnote = n
            .iter()
            .any(|node| matches!(node, InlineNode::Footnote { .. }));
        assert!(has_footnote, "expected a Footnote node, got {n:?}");
    }

    #[test]
    fn does_not_perturb_caller_parser() {
        // The AST pass runs on a clone; the caller's rendering is unaffected.
        let mut p = Parser::default();
        let before = p
            .parse("A *bold* claim.")
            .child_blocks()
            .next()
            .unwrap()
            .rendered_content()
            .map(str::to_owned);

        let _ = SubstitutionGroup::Normal.inline_nodes(Span::new("A *bold* claim."), &p, None);

        let after = p
            .parse("A *bold* claim.")
            .child_blocks()
            .next()
            .unwrap()
            .rendered_content()
            .map(str::to_owned);

        assert_eq!(before, after);
        assert_eq!(before.as_deref(), Some("A <strong>bold</strong> claim."));
    }

    /// Returns the inline AST of the first simple block, requiring capture to
    /// have produced one.
    fn block_nodes<'d>(doc: &'d crate::Document<'d>) -> &'d [InlineNode] {
        doc.child_blocks()
            .find_map(|b| match b {
                crate::blocks::Block::Simple(s) => Some(s.content()),
                _ => None,
            })
            .and_then(|c| c.inline_nodes())
            .unwrap()
    }

    #[test]
    fn capture_disabled_by_default() {
        let mut p = Parser::default();
        let doc = p.parse("A *bold* claim.");
        let simple = doc
            .child_blocks()
            .find_map(|b| match b {
                crate::blocks::Block::Simple(s) => Some(s.content()),
                _ => None,
            })
            .unwrap();

        assert!(simple.inline_nodes().is_none());
    }

    #[test]
    fn capture_via_parser_flag() {
        let mut p = Parser::default().with_inline_ast_capture();
        let doc = p.parse("A *bold* claim.");

        assert_eq!(
            block_nodes(&doc),
            &[
                InlineNode::Text("A ".into()),
                InlineNode::Styled {
                    style: InlineStyle::Strong,
                    id: None,
                    roles: vec![],
                    children: vec![InlineNode::Text("bold".into())],
                },
                InlineNode::Text(" claim.".into()),
            ]
        );
    }

    #[test]
    fn captured_footnote_number_matches_document_order() {
        // Two footnotes: the second must be number 2 in the AST, proving the
        // capture pass shares the document's counter state (not restarted).
        let mut p = Parser::default().with_inline_ast_capture();
        let doc = p.parse("First.footnote:[one]\n\nSecond.footnote:[two]");

        let second = doc
            .child_blocks()
            .filter_map(|b| match b {
                crate::blocks::Block::Simple(s) => s.content().inline_nodes(),
                _ => None,
            })
            .nth(1)
            .unwrap();

        let footnote = second
            .iter()
            .find_map(|n| match n {
                InlineNode::Footnote { index, .. } => Some(index.clone()),
                _ => None,
            })
            .flatten();

        assert_eq!(footnote.as_deref(), Some("2"));
    }

    #[test]
    fn captured_xref_is_resolved_after_parsing() {
        let mut p = Parser::default().with_inline_ast_capture();
        let doc = p.parse("[[intro]]\n== Intro\n\nSee <<intro>>.");

        // The xref lives in the second simple block ("See <<intro>>.").
        let xref = doc
            .child_blocks()
            .flat_map(|b| collect_simple(b))
            .find_map(|c| {
                c.inline_nodes()?
                    .iter()
                    .find(|n| matches!(n, InlineNode::Xref { .. }))
                    .cloned()
            })
            .unwrap();

        let InlineNode::Xref { target, href, .. } = xref else {
            unreachable!()
        };

        assert_eq!(target, "intro");
        assert!(
            href.as_deref().is_some_and(|h| h.contains("intro")),
            "expected the xref to resolve to the intro section, got {href:?}"
        );
    }

    /// Collects the `Content` of every simple block at or under `block`.
    fn collect_simple<'d>(
        block: &'d crate::blocks::Block<'d>,
    ) -> Vec<&'d crate::content::Content<'d>> {
        let mut out = vec![];
        if let crate::blocks::Block::Simple(s) = block {
            out.push(s.content());
        }
        for child in block.child_blocks() {
            out.extend(collect_simple(child));
        }
        out
    }
}
