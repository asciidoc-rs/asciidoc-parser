//! Virtual DOM representation of parsed AsciiDoc documents.
//!
//! This module provides a lightweight HTML-like representation of AsciiDoc
//! documents for testing purposes. It maps AsciiDoc block structures to their
//! HTML equivalents, enabling XPath-like queries for test assertions.

use std::{
    cell::{Cell, RefCell},
    sync::LazyLock,
    thread::LocalKey,
};

use regex::Regex;

use crate::{
    Document, HasSpan,
    blocks::{
        AdmonitionBlock, Block, Break, ColumnStyle, CompoundDelimitedBlock, ContentModel, Frame,
        Grid, HorizontalAlignment, IsBlock, ListBlock, ListItem, ListItemMarker, ListType,
        MediaBlock, Preamble, QuoteBlock, QuoteType, RawDelimitedBlock, SectionBlock, SimpleBlock,
        SimpleBlockStyle, Stripes, TableBlock, TableCellContent, TableColumn, TableRow,
        VerticalAlignment,
    },
    document::{InterpretedValue, TocMode},
};

/// The document-wide `icons` mode, which controls how callouts and callout
/// lists render. The virtual DOM builder has no document context threaded
/// through it, so this is captured once at the top of
/// [`Document::to_virtual_dom`] and read where callout lists are built.
#[derive(Clone, Copy, Eq, PartialEq)]
enum IconsMode {
    /// No `icons` attribute: callouts render as text conums and callout lists
    /// as an ordered list.
    None,

    /// `icons` set (without `font`): image-based callout icons and an icon
    /// table for callout lists.
    Image,

    /// `icons=font`: font-based callout icons and an icon table for callout
    /// lists.
    Font,
}

thread_local! {
    static ICONS_MODE: Cell<IconsMode> = const { Cell::new(IconsMode::None) };
}

/// A fully resolved table of contents, ready to render: the prebuilt section
/// list (already pruned to the configured `toclevels`), the title text (from
/// `toc-title`), and the container CSS class (from `toc-class`).
#[derive(Clone)]
struct TocData {
    /// The nested `<ul>` of section links, or `None` when the document has no
    /// sections.
    ul: Option<VirtualNode>,

    /// The TOC title text (`toc-title`).
    title: String,

    /// The CSS class on the `div` container (`toc-class`).
    class: String,
}

impl TocData {
    /// Builds the table of contents from a document's (or cell's) resolved
    /// `toc` configuration and its top-level `blocks`.
    fn build(levels: usize, title: &str, class: &str, blocks: &[Block]) -> Self {
        Self {
            ul: build_toc_ul(blocks, 1, levels),
            title: title.to_string(),
            class: class.to_string(),
        }
    }
}

thread_local! {
    /// The table of contents for the current `toc: macro` scope.
    ///
    /// The virtual DOM builder has no document context threaded through it, so
    /// when a document (or nested AsciiDoc cell) renders with `toc: macro`, its
    /// prebuilt TOC is stashed here for the duration of the body render and a
    /// `toc::[]` macro reads it from here. `Some` means a macro scope is active;
    /// `None` means no macro scope (a stray `toc::[]` then renders nothing).
    static MACRO_TOC: RefCell<Option<TocData>> = const { RefCell::new(None) };

    /// The table of contents for the current `toc: preamble` scope, consumed by
    /// the preamble while it renders. `Some` means a preamble-placed TOC is
    /// pending; `None` means none is (so the preamble renders no TOC).
    static PREAMBLE_TOC: RefCell<Option<TocData>> = const { RefCell::new(None) };
}

/// Restores a stashed-TOC thread-local to its previous value when dropped, so
/// nested AsciiDoc cells can mask the enclosing document's scope and reinstate
/// it afterward.
struct TocScopeGuard {
    key: &'static LocalKey<RefCell<Option<TocData>>>,
    prev: Option<TocData>,
}

impl Drop for TocScopeGuard {
    fn drop(&mut self) {
        self.key.with(|c| *c.borrow_mut() = self.prev.take());
    }
}

/// Sets the stashed-TOC thread-local `key` to `value` for the current scope,
/// returning a guard that restores the previous value on drop.
fn scoped_toc(
    key: &'static LocalKey<RefCell<Option<TocData>>>,
    value: Option<TocData>,
) -> TocScopeGuard {
    let prev = key.with(|c| c.replace(value));
    TocScopeGuard { key, prev }
}

/// A regular expression matching a `toc::[]` block macro line (with any
/// attributes between the brackets).
static TOC_MACRO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^toc::\[.*\]$").unwrap());

/// Returns `true` if `block` is a `toc::[]` block macro (which the parser
/// represents as a plain paragraph, since `toc` is not a media macro).
fn is_toc_macro(block: &Block) -> bool {
    matches!(block, Block::Simple(simple)
        if simple.declared_style().is_none()
            && TOC_MACRO.is_match(simple.content().original().data().trim()))
}

/// Renders a `toc::[]` block macro: the table of contents for the active
/// `toc: macro` scope, carrying the macro's id (default `toc`). Returns `None`
/// when no macro scope is active, in which case the macro renders nothing
/// (matching Asciidoctor, which drops `toc::[]` unless `toc: macro` is in
/// effect).
fn toc_macro_node(block: &Block) -> Option<VirtualNode> {
    MACRO_TOC.with(|m| {
        m.borrow()
            .as_ref()
            .map(|data| toc_block(block.id().unwrap_or("toc"), true, data))
    })
}

/// Builds the nested `<ul>` of section links for a table of contents.
///
/// `blocks` are the blocks at the current `level` (1 for the top-level
/// sections); only [`Block::Section`] children contribute entries. Each entry
/// is an `<li>` with an `<a href="#id">title</a>`, plus a nested `<ul>` of any
/// subsections down to `max_level`. Returns `None` when there are no sections.
fn build_toc_ul(blocks: &[Block], level: usize, max_level: usize) -> Option<VirtualNode> {
    if level > max_level {
        return None;
    }

    let mut ul = VirtualNode::new("ul").with_class(format!("sectlevel{level}"));

    for block in blocks {
        if let Block::Section(section) = block {
            let mut li = VirtualNode::new("li");

            let id = section.id().unwrap_or_default();
            li.children.push(
                VirtualNode::new("a")
                    .with_attribute("href", format!("#{id}"))
                    .with_text(section.section_title()),
            );

            if let Some(sub) =
                build_toc_ul(section.nested_blocks().as_slice(), level + 1, max_level)
            {
                li.children.push(sub);
            }

            ul.children.push(li);
        }
    }

    (!ul.children.is_empty()).then_some(ul)
}

/// Wraps a table-of-contents list in its container `div`.
///
/// `id` is the container id (`toc` for an automatic placement, or the `toc::[]`
/// macro's id). `title_class` adds `class="title"` to the title element, which
/// Asciidoctor does for a `toc::[]` macro rendered in the document body but not
/// for the automatic top-of-document TOC. `data` supplies the section list
/// (omitted when the document has no sections), the title text, and the
/// container CSS class.
fn toc_block(id: &str, title_class: bool, data: &TocData) -> VirtualNode {
    let mut node = VirtualNode::new("div")
        .with_id(id)
        .with_class(data.class.as_str());

    let mut title = VirtualNode::new("div")
        .with_id(format!("{id}title"))
        .with_text(data.title.as_str());
    if title_class {
        title = title.with_class("title");
    }
    node.children.push(title);

    if let Some(ul) = data.ul.clone() {
        node.children.push(ul);
    }

    node
}

/// Resolves the document's `icons` mode from its header attributes.
fn icons_mode_from_document(doc: &Document) -> IconsMode {
    for attr in doc.header().attributes() {
        if attr.name().data() == "icons" {
            return match attr.value() {
                InterpretedValue::Value(v) if v == "font" => IconsMode::Font,
                InterpretedValue::Unset => IconsMode::None,
                // `:icons:` (set, empty) or any other value enables image icons.
                _ => IconsMode::Image,
            };
        }
    }
    IconsMode::None
}

/// Decodes common HTML entities to their character equivalents.
///
/// This simulates what a browser would do when parsing HTML and accessing
/// text content via JavaScript's `textContent` or XPath's `text()`.
fn decode_html_entities(s: &str) -> String {
    let s = decode_numeric_entities(s);
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Decodes numeric character references (`&#8230;` and `&#x2026;`) to their
/// character equivalents, mirroring what a browser does when reading `text()`.
/// Asciidoctor emits typographic replacements (ellipsis, dashes, zero-width
/// spaces) as numeric references, so the test DOM must decode them to compare
/// against the expected characters.
fn decode_numeric_entities(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(amp) = rest.find("&#") {
        result.push_str(&rest[..amp]);
        let after = &rest[amp + 2..];

        let (digits, radix) = match after.strip_prefix(['x', 'X']) {
            Some(hex) => (hex, 16),
            None => (after, 10),
        };

        let end = digits.find(';');
        let parsed = end
            .map(|e| &digits[..e])
            .and_then(|d| u32::from_str_radix(d, radix).ok())
            .and_then(char::from_u32);

        match (end, parsed) {
            (Some(e), Some(ch)) => {
                result.push(ch);
                rest = &digits[e + 1..];
            }
            _ => {
                // Not a well-formed numeric reference; keep the literal `&#`.
                result.push_str("&#");
                rest = after;
            }
        }
    }

    result.push_str(rest);
    result
}

/// Parses simple HTML inline markup from text and returns a mix of text and
/// element nodes.
///
/// This handles common inline HTML elements like <strong>, <em>, <code>, etc.
/// It does not handle nested elements or attributes - just simple tags with
/// text content.
fn parse_html_content(text: &str) -> Vec<VirtualNode> {
    let mut result = Vec::new();
    let mut last_pos = 0;
    let mut i = 0;

    while i < text.len() {
        if text[i..].starts_with('<') {
            // Try to parse an HTML element.
            if let Some((element, new_pos)) = try_parse_element(text, i) {
                // Add any text before this element.
                if i > last_pos {
                    let text_content = &text[last_pos..i];
                    if !text_content.is_empty() {
                        result.push(VirtualNode::new("text").with_text(text_content));
                    }
                }

                // Add the element.
                result.push(element);

                // Move forward.
                i = new_pos;
                last_pos = new_pos;
                continue;
            }
        }
        i += 1;
    }

    // Add any remaining text.
    if last_pos < text.len() {
        let remaining = &text[last_pos..];
        if !remaining.is_empty() {
            result.push(VirtualNode::new("text").with_text(remaining));
        }
    }

    // If we never created any nodes, create a text node.
    if result.is_empty() && !text.is_empty() {
        result.push(VirtualNode::new("text").with_text(text));
    }

    result
}

/// Attempts to parse an HTML element starting at position `pos`.
/// Returns the element and the position after the closing tag if successful.
fn try_parse_element(text: &str, pos: usize) -> Option<(VirtualNode, usize)> {
    if !text[pos..].starts_with('<') {
        return None;
    }

    // Find the end of the opening tag.
    let tag_end = text[pos + 1..].find('>')?;
    let tag_content = &text[pos + 1..pos + 1 + tag_end];
    let tag_name = extract_tag_name(tag_content)?;

    // Void elements (e.g. `<img>`, `<br>`) and explicitly self-closing tags
    // have no closing tag; emit them as childless element nodes (preserving
    // their attributes) and advance past the opening tag.
    const VOID_ELEMENTS: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "track", "wbr",
    ];
    if tag_content.ends_with('/') || VOID_ELEMENTS.iter().any(|v| *v == tag_name) {
        let after_opening = pos + 1 + tag_end + 1;
        let element = apply_tag_attributes(VirtualNode::new(tag_name), tag_content);
        return Some((element, after_opening));
    }

    // Find the closing tag.
    let after_opening = pos + 1 + tag_end + 1;
    let closing_tag = format!("</{tag_name}>");
    let close_pos = text[after_opening..].find(&closing_tag)?;

    // Extract content between tags.
    let content = &text[after_opening..after_opening + close_pos];
    let after_closing = after_opening + close_pos + closing_tag.len();

    // Create the element.
    let element = if content.contains('<') {
        // Nested HTML - recursively parse.
        VirtualNode::new(tag_name).with_children(parse_html_content(content))
    } else {
        // Plain text content.
        VirtualNode::new(tag_name).with_text(content)
    };

    // Capture the opening tag's attributes (id, class, href, etc.) so that
    // attribute predicates like `[@href="#x"]` can match inline elements.
    let element = apply_tag_attributes(element, tag_content);

    Some((element, after_closing))
}

/// Matches `name="value"` attribute pairs in an opening tag. Renderer output
/// always uses double quotes, so single-quoted values are not handled.
static HTML_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"([a-zA-Z_:][-a-zA-Z0-9_:.]*)\s*=\s*"([^"]*)""#).unwrap()
});

/// Parses the attributes from an opening tag's content and applies them to
/// `node`, routing `id` and `class` to their dedicated fields and everything
/// else into the generic attribute map.
fn apply_tag_attributes(mut node: VirtualNode, tag_content: &str) -> VirtualNode {
    // Skip the tag name; only the remainder can contain attributes.
    let attrs = tag_content
        .trim()
        .split_once(char::is_whitespace)
        .map(|(_, rest)| rest)
        .unwrap_or("");

    for caps in HTML_ATTR.captures_iter(attrs) {
        let name = &caps[1];
        let value = caps[2].to_string();

        match name {
            "id" => node.id = Some(value),
            "class" => {
                for class in value.split_whitespace() {
                    node.classes.push(class.to_string());
                }
            }
            _ => {
                node.attributes.insert(name.to_string(), value);
            }
        }
    }

    node
}

/// Extracts the tag name from an opening tag string (without the < and >).
fn extract_tag_name(tag_content: &str) -> Option<String> {
    let tag_content = tag_content.trim();
    if tag_content.is_empty() || tag_content.starts_with('/') {
        return None;
    }

    // Extract tag name (before any whitespace or attributes).
    let tag_name = tag_content
        .split_whitespace()
        .next()
        .unwrap_or(tag_content)
        .trim_end_matches('/');

    if tag_name.is_empty() {
        None
    } else {
        Some(tag_name.to_string())
    }
}

/// A virtual DOM node representing an HTML-like element.
///
/// This structure is built from a parsed `Document` and maps AsciiDoc blocks
/// to their HTML equivalents for testing purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualNode {
    /// HTML tag name (e.g., "ul", "li", "p", "div").
    pub tag: String,

    /// CSS classes applied to this element.
    pub classes: Vec<String>,

    /// Element ID attribute, if any.
    pub id: Option<String>,

    /// Text content of this element (for leaf nodes).
    pub text: Option<String>,

    /// Other HTML attributes (e.g., "start", "type", etc.).
    pub attributes: std::collections::HashMap<String, String>,

    /// Child elements.
    pub children: Vec<VirtualNode>,
}

#[allow(dead_code)] // TEMPORARY while building
impl VirtualNode {
    /// Creates a new virtual node with the specified tag.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            classes: Vec::new(),
            id: None,
            text: None,
            attributes: std::collections::HashMap::new(),
            children: Vec::new(),
        }
    }

    /// Adds a CSS class to this node.
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// Adds multiple CSS classes to this node.
    pub fn with_classes(mut self, classes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.classes.extend(classes.into_iter().map(Into::into));
        self
    }

    /// Sets the ID of this node.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets an arbitrary HTML attribute on this node.
    pub fn with_attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(name.into(), value.into());
        self
    }

    /// Sets the text content of this node.
    ///
    /// Decodes HTML entities to match what a browser's text content would show.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(decode_html_entities(&text.into()));
        self
    }

    /// Sets the text content and parses any HTML inline elements.
    ///
    /// This will parse HTML tags like <strong>, <em>, <code>, etc. and create
    /// child nodes for them.
    pub fn with_html_content(mut self, text: impl Into<String>) -> Self {
        let content = text.into();

        // Check if there's any HTML to parse.
        if content.contains('<') {
            self.children = parse_html_content(&content);
        } else {
            // No HTML - just set as plain text.
            self.text = Some(decode_html_entities(&content));
        }

        self
    }

    /// Adds a child node.
    pub fn with_child(mut self, child: VirtualNode) -> Self {
        self.children.push(child);
        self
    }

    /// Adds multiple child nodes.
    pub fn with_children(mut self, children: impl IntoIterator<Item = VirtualNode>) -> Self {
        self.children.extend(children);
        self
    }
}

/// Trait for converting AsciiDoc structures to virtual DOM nodes.
pub trait ToVirtualDom {
    /// Converts this structure to a virtual DOM node.
    fn to_virtual_dom(&self) -> VirtualNode;
}

impl ToVirtualDom for Document<'_> {
    fn to_virtual_dom(&self) -> VirtualNode {
        // Capture the document-wide icons mode so callout lists (built without
        // any document context) can render an icon table when appropriate.
        ICONS_MODE.with(|m| m.set(icons_mode_from_document(self)));

        let mut node = VirtualNode::new("div").with_class("document");

        // Add document ID if present.
        if let Some(id) = self.id() {
            node = node.with_id(id);
        }

        // The document title renders as an `<h1>` when it is shown (the
        // effective `showtitle`/`notitle` state).
        if self.show_doctitle()
            && let Some(title) = self.doctitle()
        {
            node.children.push(VirtualNode::new("h1").with_text(title));
        }

        // Resolve the table of contents from the `toc` family of attributes.
        // `auto`/`left`/`right` render the TOC at the top of the body;
        // `preamble` defers it to the preamble; `macro` stashes it for a
        // `toc::[]` macro in the body to pick up. (The side-column styling of
        // `left`/`right` needs the standalone HTML body framing this embeddable
        // virtual DOM does not model, so they render like `auto` here, which is
        // also Asciidoctor's documented fallback for embeddable output.)
        let toc_mode = self.toc_mode();
        let toc_data = toc_mode.is_enabled().then(|| {
            TocData::build(
                self.toc_levels(),
                self.toc_title(),
                self.toc_class(),
                self.nested_blocks().as_slice(),
            )
        });

        // Guards keeping a stashed TOC alive for the duration of the body
        // render (dropped, and the thread-locals restored, when this returns).
        let mut _macro_scope = None;
        let mut _preamble_scope = None;
        match toc_mode {
            TocMode::Auto | TocMode::Left | TocMode::Right => {
                if let Some(data) = &toc_data {
                    node.children.push(toc_block("toc", false, data));
                }
            }
            TocMode::Preamble => _preamble_scope = Some(scoped_toc(&PREAMBLE_TOC, toc_data)),
            TocMode::Macro => _macro_scope = Some(scoped_toc(&MACRO_TOC, toc_data)),
            TocMode::Disabled => {}
        }

        // Add child blocks, including block titles as separate siblings.
        for block in self.nested_blocks() {
            add_block_with_title(&mut node, block);
        }

        node
    }
}

/// Adds a block to the parent node, including its title as a separate sibling
/// element if present.
///
/// NOTE: Some block types (like lists) handle their titles internally, so we
/// skip adding a separate title element for those.
fn add_block_with_title<'a>(parent: &mut VirtualNode, block: &'a Block<'a>) {
    // A `toc::[]` macro renders the table of contents (when a `toc: macro` scope
    // is active) rather than a paragraph, and never carries a separate title.
    if is_toc_macro(block) {
        if let Some(toc) = toc_macro_node(block) {
            parent.children.push(toc);
        }
        return;
    }

    // Check if this block type handles its own title internally. Lists render
    // their title inside the list wrapper; tables render it as a <caption>;
    // admonitions render it inside the content cell; a collapsible block renders
    // it as the `<summary>` toggle text; a sidebar renders it as the first child
    // of its `div.content` wrapper; an example block renders it as a numbered
    // caption before its `div.content` wrapper.
    let handles_title_internally = is_collapsible(block)
        || is_sidebar(block)
        || is_example(block)
        || is_open(block)
        || matches!(
            block,
            Block::List(_) | Block::Table(_) | Block::Admonition(_) | Block::Quote(_)
        );

    // Add title as a separate sibling element if the block doesn't handle it
    // internally.
    if !handles_title_internally && let Some(title) = block.title() {
        // Add title as a separate div element with class="title".
        let title_node = VirtualNode::new("div").with_class("title").with_text(title);
        parent.children.push(title_node);
    }

    // Check if this is a paragraph that needs to be wrapped in div.paragraph.
    // Asciidoctor wraps top-level paragraphs in <div
    // class="paragraph"><p>...</p></div>.
    if let Block::Simple(simple) = block
        && simple.declared_style().is_none()
        && simple.style() == SimpleBlockStyle::Paragraph
    {
        let mut p_node = block.to_virtual_dom();
        let mut wrapper = VirtualNode::new("div").with_class("paragraph");

        // Roles and ID belong on the wrapper div, not the inner <p>.
        wrapper.classes.append(&mut p_node.classes);
        if p_node.id.is_some() {
            wrapper.id = p_node.id.take();
        }

        wrapper.children.push(p_node);
        parent.children.push(wrapper);
    } else {
        // Add the block itself (which will handle its own title if applicable).
        parent.children.push(block.to_virtual_dom());
    }
}

impl ToVirtualDom for Block<'_> {
    fn to_virtual_dom(&self) -> VirtualNode {
        // A collapsible example block or paragraph renders as an HTML disclosure
        // element (`<details>`/`<summary>`) rather than its usual example-block
        // markup.
        if is_collapsible(self) {
            return collapsible_to_node(self);
        }

        // A sidebar block (the `****` structural container or the `[sidebar]`
        // paragraph style) renders as `div.sidebarblock > div.content`, with its
        // title and content nested inside the content wrapper.
        if is_sidebar(self) {
            return sidebar_to_node(self);
        }

        // An example block (the `====` structural container or the `[example]`
        // paragraph style) renders as `div.exampleblock > div.content`, with a
        // numbered caption preceding the content wrapper.
        if is_example(self) {
            return example_to_node(self);
        }

        match self {
            Block::Simple(simple) => {
                // Comment blocks should not be rendered.
                if simple.declared_style() == Some("comment") {
                    return VirtualNode::new("comment");
                }

                let mut node = simple_block_to_node(simple);

                // For literal and verse blocks, add a <pre> element containing the content.
                if simple.style() == SimpleBlockStyle::Literal
                    || simple.declared_style() == Some("literal")
                    || simple.declared_style() == Some("verse")
                {
                    let pre_node =
                        VirtualNode::new("pre").with_text(simple.content().rendered().to_string());
                    node = node.with_child(pre_node);
                }
                node
            }

            Block::List(list) => list_block_to_node(list),
            Block::ListItem(item) => list_item_to_node(item),

            Block::Section(section) => {
                let mut node = section_to_node(section);
                // Add section title as heading element.
                // Section levels are 1-5, which map to h2-h6.
                let heading_level = (section.level() + 1).min(6);
                let heading_tag = format!("h{}", heading_level);

                let mut title_node =
                    VirtualNode::new(heading_tag).with_text(section.section_title());

                // Add the section ID to the heading element if present.
                if let Some(id) = section.id() {
                    title_node = title_node.with_id(id);
                }

                node.children.insert(0, title_node);
                node
            }

            Block::Media(media) => media_to_node(media),
            Block::RawDelimited(raw) => raw_delimited_to_node(raw),
            Block::CompoundDelimited(compound) => compound_delimited_to_node(compound),
            Block::Admonition(admonition) => admonition_to_node(admonition),
            Block::Quote(quote) => quote_to_node(quote),
            Block::Table(table) => table_to_node(table),
            Block::Preamble(preamble) => preamble_to_node(preamble),
            Block::Break(break_) => break_to_node(break_),
            Block::DocumentAttribute(_) => {
                // Document attributes don't render in HTML.
                VirtualNode::new("comment")
            }
        }
    }
}

fn simple_block_to_node<'a>(block: &'a SimpleBlock<'a>) -> VirtualNode {
    let declared_style = block.declared_style();
    let block_style = block.style();

    // Determine tag and classes based on both declared style and block style.
    let (tag, wrapper_classes) =
        if block_style == SimpleBlockStyle::Literal || declared_style == Some("literal") {
            ("div", vec!["literalblock"])
        } else {
            match declared_style {
                Some("paragraph") | None => ("p", vec![]),
                Some("verse") => ("div", vec!["verseblock"]),
                Some("quote") => ("div", vec!["quoteblock"]),
                Some("sidebar") => ("div", vec!["sidebarblock"]),
                Some("example") => ("div", vec!["exampleblock"]),
                Some("open") => ("div", vec!["openblock"]),
                Some("pass") => ("div", vec!["passblock"]),
                _ => ("p", vec![]),
            }
        };

    let mut node = VirtualNode::new(tag);

    for class in wrapper_classes {
        node = node.with_class(class);
    }

    for role in block.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = block.id() {
        node = node.with_id(id);
    }

    // Extract text content for paragraphs (only for <p> tags).
    if tag == "p" {
        node = node.with_html_content(block.content().rendered().to_string());
    }

    node
}

fn list_block_to_node<'a>(list: &'a ListBlock<'a>) -> VirtualNode {
    // When icons are enabled, a callout list renders as an icon table rather
    // than an ordered list (matching Asciidoctor's `convert_colist`).
    if list.type_() == ListType::Callout {
        let icons = ICONS_MODE.with(|m| m.get());
        if icons != IconsMode::None {
            return colist_icon_table_to_node(list, icons);
        }
    }

    // Horizontal description lists render as tables instead of dl elements.
    let is_horizontal =
        list.type_() == ListType::Description && list.declared_style() == Some("horizontal");

    let (list_tag, base_class) = match list.type_() {
        ListType::Unordered => ("ul", "ulist"),
        ListType::Ordered => ("ol", "olist"),
        ListType::Description => {
            if is_horizontal {
                ("table", "hdlist")
            } else {
                ("dl", "dlist")
            }
        }
        ListType::Callout => ("ol", "colist"),
    };

    let mut list_element = VirtualNode::new(list_tag);

    // For ordered lists, add style class to the list element based on marker
    // length, but only if no explicit style is declared.
    if list.type_() == ListType::Ordered
        && list.declared_style().is_none()
        && let Some(style) = list.marker_style()
    {
        list_element = list_element.with_class(style);
    }

    // For ordered lists whose explicit numbering does not start at 1, emit the
    // implicit `start` attribute (matching Asciidoctor).
    if list.type_() == ListType::Ordered
        && let Some(Block::ListItem(first)) = list.nested_blocks().next()
        && let Some(ordinal) = first.list_item_marker().ordinal_value()
        && ordinal != 1
    {
        list_element = list_element.with_attribute("start", ordinal.to_string());
    }

    // Add all named attributes from the attrlist to the list element.
    if let Some(attrlist) = list.attrlist() {
        for attr in attrlist.attributes() {
            if let Some(attr_name) = attr.name() {
                list_element = list_element.with_attribute(attr_name, attr.value());
            }
        }
    }

    // Add options as boolean attributes on the list element.
    for option in list.options() {
        list_element = list_element.with_attribute(option, "");
    }

    // Add style class to the list element if present.
    // Skip for horizontal dlists since they use a different rendering.
    if !is_horizontal && let Some(style) = list.declared_style() {
        list_element = list_element.with_class(style);
    }

    for item in list.nested_blocks() {
        // For description lists, we need to create two peer nodes: dt and dd
        // (or tr/td for horizontal lists).
        if list.type_() == ListType::Description {
            if let Block::ListItem(list_item) = item {
                // Create dt node for the term.
                if let ListItemMarker::DefinedTerm { term, .. } = list_item.list_item_marker() {
                    if is_horizontal {
                        // Horizontal description lists render as table rows.
                        let mut tr_node = VirtualNode::new("tr");

                        let td_term = VirtualNode::new("td")
                            .with_class("hdlist1")
                            .with_html_content(term.rendered().to_string());
                        tr_node.children.push(td_term);

                        let mut td_def = VirtualNode::new("td").with_class("hdlist2");
                        let nested = list_item.nested_blocks().collect::<Vec<_>>();
                        for child in &nested {
                            td_def.children.push(child.to_virtual_dom());
                        }
                        tr_node.children.push(td_def);

                        list_element.children.push(tr_node);
                    } else {
                        let mut dt_node = VirtualNode::new("dt");

                        for role in list_item.roles() {
                            dt_node = dt_node.with_class(role);
                        }

                        if let Some(id) = list_item.id() {
                            dt_node = dt_node.with_id(id);
                        }

                        // Set the term text.
                        dt_node = dt_node.with_html_content(term.rendered().to_string());
                        list_element.children.push(dt_node);

                        // Create dd node for the definition, but only if the item has content.
                        // Multiple consecutive terms can share a single definition.
                        let nested = list_item.nested_blocks().collect::<Vec<_>>();

                        if !nested.is_empty() {
                            let mut dd_node = VirtualNode::new("dd");

                            let has_multiple_blocks = nested.len() > 1;

                            // Check if the first block was attached via list
                            // continuation (+). When content is from continuation,
                            // paragraphs should be wrapped in div.paragraph.
                            let first_block_from_continuation =
                                nested.first().is_some_and(|first_block| {
                                    let item_span = list_item.span();
                                    let marker_span = list_item.list_item_marker().span();

                                    let marker_end_offset =
                                        marker_span.byte_offset() + marker_span.data().len();

                                    let first_block_offset = first_block.span().byte_offset();

                                    let item_start = item_span.byte_offset();

                                    if first_block_offset > marker_end_offset
                                        && marker_end_offset >= item_start
                                    {
                                        let start = marker_end_offset - item_start;
                                        let end = first_block_offset - item_start;

                                        if end <= item_span.data().len() {
                                            let between = &item_span.data()[start..end];
                                            between.lines().any(|line| line.trim() == "+")
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                });

                            for (index, child) in nested.iter().enumerate() {
                                let child_vdom = child.to_virtual_dom();

                                // Wrap paragraphs in div.paragraph when they
                                // appear after other blocks or when the first
                                // block was attached via continuation.
                                let should_wrap = child_vdom.tag == "p"
                                    && child_vdom.classes.is_empty()
                                    && ((has_multiple_blocks && index > 0)
                                        || (index == 0 && first_block_from_continuation));

                                if should_wrap {
                                    let wrapper = VirtualNode::new("div")
                                        .with_class("paragraph")
                                        .with_child(child_vdom);
                                    dd_node.children.push(wrapper);
                                } else {
                                    dd_node.children.push(child_vdom);
                                }
                            }

                            list_element.children.push(dd_node);
                        }
                    }
                }
            }
        } else {
            list_element.children.push(item.to_virtual_dom());
        }
    }

    // Wrap the list in a div container (matching Asciidoctor's HTML structure).
    let mut wrapper = VirtualNode::new("div").with_class(base_class);

    // For ordered and callout lists, add style class to the wrapper based on
    // marker length, but only if no explicit style is declared.
    if matches!(list.type_(), ListType::Ordered | ListType::Callout)
        && list.declared_style().is_none()
        && let Some(style) = list.marker_style()
    {
        wrapper = wrapper.with_class(style);
    }

    // Add style class to the wrapper if present (explicit style overrides marker
    // style). Skip for horizontal dlists since wrapper already has hdlist class.
    if !is_horizontal && let Some(style) = list.declared_style() {
        wrapper = wrapper.with_class(style);
    }

    for role in list.roles() {
        wrapper = wrapper.with_class(role);
    }

    if let Some(id) = list.id() {
        wrapper = wrapper.with_id(id);
    }

    // Add block title if present (inside the wrapper, before the list element).
    if let Some(title) = list.title() {
        let title_node = VirtualNode::new("div").with_class("title").with_text(title);
        wrapper.children.push(title_node);
    }

    wrapper.children.push(list_element);
    wrapper
}

/// Renders a callout list as an icon table (`div.colist > table`), matching
/// Asciidoctor's `convert_colist` when the `icons` attribute is set.
fn colist_icon_table_to_node<'a>(list: &'a ListBlock<'a>, icons: IconsMode) -> VirtualNode {
    let mut wrapper = VirtualNode::new("div").with_class("colist");

    // The marker style (always "arabic" for callout lists) is added to the
    // wrapper, producing `<div class="colist arabic">`.
    if let Some(style) = list.marker_style() {
        wrapper = wrapper.with_class(style);
    }

    for role in list.roles() {
        wrapper = wrapper.with_class(role);
    }

    if let Some(id) = list.id() {
        wrapper = wrapper.with_id(id);
    }

    if let Some(title) = list.title() {
        wrapper
            .children
            .push(VirtualNode::new("div").with_class("title").with_text(title));
    }

    let mut table = VirtualNode::new("table");

    for (index, item) in list.nested_blocks().enumerate() {
        let num = index + 1;

        let mut row = VirtualNode::new("tr");

        // Icon cell.
        let mut icon_cell = VirtualNode::new("td");
        match icons {
            IconsMode::Font => {
                icon_cell.children.push(
                    VirtualNode::new("i")
                        .with_class("conum")
                        .with_attribute("data-value", num.to_string()),
                );
                icon_cell
                    .children
                    .push(VirtualNode::new("b").with_text(num.to_string()));
            }
            IconsMode::Image => {
                icon_cell.children.push(
                    VirtualNode::new("img")
                        .with_attribute("src", format!("./images/icons/callouts/{num}.png"))
                        .with_attribute("alt", num.to_string()),
                );
            }
            IconsMode::None => {}
        }
        row.children.push(icon_cell);

        // Text cell: the callout list item's annotation text.
        let mut text_cell = VirtualNode::new("td");
        if let Some(text) = item
            .nested_blocks()
            .next()
            .and_then(|b| b.rendered_content())
        {
            text_cell = text_cell.with_html_content(text);
        }
        row.children.push(text_cell);

        table.children.push(row);
    }

    wrapper.children.push(table);
    wrapper
}

fn list_item_to_node<'a>(item: &'a ListItem<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("li");

    for role in item.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = item.id() {
        node = node.with_id(id);
    }

    let nested = item.nested_blocks().collect::<Vec<_>>();
    let has_multiple_blocks = nested.len() > 1;

    for (index, child) in nested.iter().enumerate() {
        let child_vdom = child.to_virtual_dom();

        // Wrap paragraphs in div.paragraph when they appear after other blocks in the
        // list item. This matches Asciidoctor's HTML output for list
        // continuations. The first paragraph block is never wrapped, only
        // subsequent ones.
        if has_multiple_blocks
            && index > 0
            && child_vdom.tag == "p"
            && child_vdom.classes.is_empty()
        {
            let wrapper = VirtualNode::new("div")
                .with_class("paragraph")
                .with_child(child_vdom);
            node.children.push(wrapper);
        } else {
            node.children.push(child_vdom);
        }
    }

    node
}

fn section_to_node<'a>(section: &'a SectionBlock<'a>) -> VirtualNode {
    let class = format!("sect{}", section.level());
    let mut node = VirtualNode::new("div").with_class(class);

    for role in section.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = section.id() {
        node = node.with_id(id);
    }

    // TODO: Section title heading is added in the Block::Section match arm
    // using section.level() to determine the heading level (h2-h6) and
    // section.section_title() to get the rendered title text.

    // Add nested blocks, handling block titles as separate siblings.
    for child in section.nested_blocks() {
        add_block_with_title(&mut node, child);
    }

    node
}

fn media_to_node<'a>(media: &'a MediaBlock<'a>) -> VirtualNode {
    // Media blocks render as <div class="imageblock"> or similar.
    let context = media.raw_context();
    let class = format!("{}block", context.as_ref());

    let mut node = VirtualNode::new("div").with_class(class);

    for role in media.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = media.id() {
        node = node.with_id(id);
    }

    // TODO: Media blocks typically contain an <img> or similar element.
    // We'll add this when we need more detailed media representation.

    node
}

fn raw_delimited_to_node<'a>(raw: &'a RawDelimitedBlock<'a>) -> VirtualNode {
    let context = raw.raw_context();

    let (tag, classes): (&str, Vec<String>) = match context.as_ref() {
        "listing" => ("div", vec!["listingblock".to_string()]),
        "literal" => ("div", vec!["literalblock".to_string()]),
        "comment" => ("comment", vec![]),
        _ => ("div", vec![format!("{}block", context.as_ref())]),
    };

    let mut node = VirtualNode::new(tag);
    for class in classes {
        node = node.with_class(class);
    }

    for role in raw.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = raw.id() {
        node = node.with_id(id);
    }

    // Add block title if present.
    if let Some(title) = raw.title() {
        let title_node = VirtualNode::new("div").with_class("title").with_text(title);
        node.children.push(title_node);
    }

    if tag != "comment" {
        // Check if this is a source block by looking for style="source" in attrlist.
        let is_source_block = raw
            .attrlist()
            .and_then(|attrlist| attrlist.attributes().next())
            .map(|attr| attr.value() == "source")
            .unwrap_or(false);

        if is_source_block {
            // For source blocks, create pre > code structure.
            let mut code = VirtualNode::new("code");

            // Add data-lang attribute if language is specified (second positional
            // attribute).
            if let Some(attrlist) = raw.attrlist() {
                let mut attrs = attrlist.attributes();
                // Skip first attribute (style="source").
                attrs.next();
                // Second attribute is the language.
                if let Some(lang_attr) = attrs.next() {
                    code = code.with_attribute("data-lang", lang_attr.value());
                }
            }

            // Add the block's rendered content to the code element, parsing any
            // inline HTML (e.g. callout conums) into child nodes.
            if let Some(content) = raw.rendered_content() {
                code = code.with_html_content(content);
            }

            let pre = VirtualNode::new("pre").with_child(code);
            node.children.push(pre);
        } else {
            let mut pre = VirtualNode::new("pre");
            if let Some(content) = raw.rendered_content() {
                pre = pre.with_html_content(content);
            }
            node.children.push(pre);
        }
    }

    node
}

/// Renders a compound delimited block, matching Asciidoctor's HTML output.
///
/// In practice this only ever renders an open block (`div.openblock`): the
/// other compound containers are intercepted earlier — a sidebar by
/// [`is_sidebar`], an example by [`is_example`], a quote/verse masquerade by
/// [`QuoteBlock`], an admonition masquerade by [`AdmonitionBlock`], and a
/// verbatim masquerade (`source`/`listing`/`literal`) by [`RawDelimitedBlock`].
///
/// The outer `div.openblock` carries the block's id and roles. An optional
/// title is rendered as a `div.title` placed *before* the content wrapper
/// (unlike a sidebar, whose title sits inside the content). The nested blocks
/// are wrapped in a `div.content`, each routed through [`add_block_with_title`]
/// so a child's own title surfaces as a sibling `div.title` and a child
/// paragraph is wrapped in `div.paragraph`.
fn compound_delimited_to_node<'a>(compound: &'a CompoundDelimitedBlock<'a>) -> VirtualNode {
    let context = compound.raw_context();
    let class = format!("{}block", context.as_ref());

    let mut node = VirtualNode::new("div").with_class(class);

    for role in compound.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = compound.id() {
        node = node.with_id(id);
    }

    // The title, when present, precedes the content wrapper.
    if let Some(title) = compound.title() {
        node.children
            .push(VirtualNode::new("div").with_class("title").with_text(title));
    }

    let mut content = VirtualNode::new("div").with_class("content");
    for child in compound.nested_blocks() {
        add_block_with_title(&mut content, child);
    }
    node.children.push(content);

    node
}

/// Returns `true` if `block` is a collapsible block: an example block (or
/// example-styled paragraph) carrying the `collapsible` option.
///
/// Asciidoctor keeps such a block's context as `example` and renders it as an
/// HTML disclosure element rather than an example block. The same option on any
/// other context (a sidebar, an open block, a plain paragraph, etc.) is
/// ignored, so the resolved context being `example` is the single gate for the
/// collapsible rendering. This also distinguishes a collapsible paragraph
/// (`[example%collapsible]`, whose `example` style resolves to the `example`
/// context) from a bare `[%collapsible]` paragraph (which has no `example`
/// style and is therefore not collapsible).
fn is_collapsible<'a>(block: &'a Block<'a>) -> bool {
    block.resolved_context().as_ref() == "example" && block.has_option("collapsible")
}

/// Renders a collapsible block as an HTML disclosure element.
///
/// The block's title becomes the `<summary>` toggle text, defaulting to
/// `Details` when no title is set. The `open` option adds the boolean `open`
/// attribute so the disclosure starts expanded. The block's id and roles travel
/// to the `<details>` element, mirroring Asciidoctor's HTML output. Unlike an
/// example block, a collapsible block is never numbered and carries no caption
/// prefix on its toggle text.
fn collapsible_to_node<'a>(block: &'a Block<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("details");

    // The `open` option expands the disclosure by default.
    if block.has_option("open") {
        node = node.with_attribute("open", "");
    }

    if let Some(id) = block.id() {
        node = node.with_id(id);
    }

    for role in block.roles() {
        node = node.with_class(role);
    }

    // The toggle text is the block's title, or the default caption "Details".
    let summary_text = block.title().unwrap_or("Details");
    node.children.push(
        VirtualNode::new("summary")
            .with_class("title")
            .with_text(summary_text),
    );

    let mut content = VirtualNode::new("div").with_class("content");

    match block.content_model() {
        // A collapsible (example) block encloses other blocks.
        ContentModel::Compound => {
            for child in block.nested_blocks() {
                add_block_with_title(&mut content, child);
            }
        }

        // A collapsible paragraph holds inline content, rendered directly inside
        // the content wrapper without a wrapping paragraph (matching
        // Asciidoctor's output for the example paragraph style).
        _ => {
            if let Some(rendered) = block.rendered_content() {
                content = content.with_html_content(rendered);
            }
        }
    }

    node.children.push(content);
    node
}

/// Returns `true` if `block` is a sidebar block: the `****` structural
/// container or a paragraph carrying the `sidebar` style, both of which resolve
/// to the `sidebar` context.
fn is_sidebar<'a>(block: &'a Block<'a>) -> bool {
    block.resolved_context().as_ref() == "sidebar"
}

/// Returns `true` if `block` is an open block: the `--` structural container,
/// rendered by [`compound_delimited_to_node`] as `div.openblock`.
///
/// Only a delimited open block (a [`Block::CompoundDelimited`] resolving to the
/// `open` context) qualifies. Every masquerade style on a `--` block is
/// intercepted earlier — by [`is_sidebar`], [`is_example`], `QuoteBlock`,
/// `AdmonitionBlock`, or `RawDelimitedBlock` — so a `CompoundDelimited` block
/// that reaches here always resolves to `open`. Like a sidebar, an open block
/// renders its own title (as a `div.title` before its content wrapper), so
/// [`add_block_with_title`] must not also emit one.
fn is_open<'a>(block: &'a Block<'a>) -> bool {
    matches!(block, Block::CompoundDelimited(_)) && block.resolved_context().as_ref() == "open"
}

/// Renders a sidebar block, matching Asciidoctor's HTML output.
///
/// The outer `div.sidebarblock` carries the block's id and roles; its sole
/// child is a `div.content` wrapper. A title, if present, is rendered as the
/// first child of the content wrapper (a `div.title`), unlike an example block
/// whose title is a numbered caption placed outside the content. A delimited
/// sidebar encloses other blocks (rendered through `add_block_with_title` so
/// their own titles and paragraph wrappers surface); a sidebar paragraph holds
/// inline content rendered directly inside the content wrapper without a `<p>`
/// wrapper.
fn sidebar_to_node<'a>(block: &'a Block<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("div").with_class("sidebarblock");

    for role in block.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = block.id() {
        node = node.with_id(id);
    }

    let mut content = VirtualNode::new("div").with_class("content");

    match block.content_model() {
        // A delimited sidebar encloses other blocks.
        ContentModel::Compound => {
            for child in block.nested_blocks() {
                add_block_with_title(&mut content, child);
            }
        }

        // A sidebar paragraph holds inline content, rendered directly inside the
        // content wrapper without a wrapping paragraph (matching Asciidoctor's
        // output for the sidebar paragraph style).
        _ => {
            if let Some(rendered) = block.rendered_content() {
                content = content.with_html_content(rendered);
            }
        }
    }

    // The title, when present, is the first child of the content wrapper. It is
    // inserted *after* the body content because `with_html_content` replaces the
    // children vector when the rendered content contains inline markup; pushing
    // the title first would let that replacement silently drop it.
    if let Some(title) = block.title() {
        content.children.insert(
            0,
            VirtualNode::new("div").with_class("title").with_text(title),
        );
    }

    node.children.push(content);
    node
}

/// Returns `true` if `block` is an example block: the `====` delimited
/// container or a paragraph carrying the `example` style, both of which resolve
/// to the `example` context.
///
/// A collapsible example block resolves to the same context but renders as an
/// HTML disclosure element instead; it is handled by [`is_collapsible`] before
/// this check is reached, so it never falls through to the example rendering.
fn is_example<'a>(block: &'a Block<'a>) -> bool {
    block.resolved_context().as_ref() == "example"
}

/// Renders an example block, matching Asciidoctor's HTML output.
///
/// The outer `div.exampleblock` carries the block's id and roles; its body is
/// wrapped in a `div.content`. A delimited example encloses other blocks
/// (rendered through `add_block_with_title` so their own titles and paragraph
/// wrappers surface); an example paragraph holds inline content rendered
/// directly inside the content wrapper without a `<p>` wrapper.
///
/// A titled example block is rendered with a `div.title` caption placed
/// *before* the content wrapper — unlike a sidebar, whose title sits inside the
/// content. The caption prefix and number are computed during parsing and
/// stored on the block (see [`IsBlock::caption`]); when present the prefix is
/// prepended to the title (e.g. "Example 1. Onomatopoeia"), otherwise the bare
/// title is shown.
fn example_to_node<'a>(block: &'a Block<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("div").with_class("exampleblock");

    for role in block.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = block.id() {
        node = node.with_id(id);
    }

    // The caption prefix (e.g. "Example 1. ") was assigned during parsing; the
    // converter simply prepends it to the title.
    if let Some(title) = block.title() {
        let caption_text = match block.caption() {
            Some(prefix) => format!("{prefix}{title}"),
            None => title.to_string(),
        };
        node.children.push(
            VirtualNode::new("div")
                .with_class("title")
                .with_text(caption_text),
        );
    }

    let mut content = VirtualNode::new("div").with_class("content");

    match block.content_model() {
        // A delimited example encloses other blocks.
        ContentModel::Compound => {
            for child in block.nested_blocks() {
                add_block_with_title(&mut content, child);
            }
        }

        // An example paragraph holds inline content, rendered directly inside the
        // content wrapper without a wrapping paragraph.
        _ => {
            if let Some(rendered) = block.rendered_content() {
                content = content.with_html_content(rendered);
            }
        }
    }

    node.children.push(content);
    node
}

fn admonition_to_node<'a>(admonition: &'a AdmonitionBlock<'a>) -> VirtualNode {
    // The outer wrapper carries `admonitionblock`, the admonition name (e.g.
    // `note`), then any roles. The ID, if any, is set on this element.
    let mut node = VirtualNode::new("div")
        .with_class("admonitionblock")
        .with_class(admonition.name());

    for role in admonition.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = admonition.id() {
        node = node.with_id(id);
    }

    // The label/icon and content are laid out in a single-row table.
    let mut icon_cell = VirtualNode::new("td").with_class("icon");
    if admonition.icons_font() {
        // With font icons enabled, the label is rendered as a Font Awesome icon
        // whose `title` carries the caption text.
        icon_cell = icon_cell.with_child(
            VirtualNode::new("i")
                .with_class("fa")
                .with_class(format!("icon-{}", admonition.name()))
                .with_attribute("title", admonition.label()),
        );
    } else {
        // Otherwise the caption text (or emoji glyph) is shown as a title.
        icon_cell = icon_cell.with_child(
            VirtualNode::new("div")
                .with_class("title")
                .with_text(admonition.label()),
        );
    }

    let mut content_cell = VirtualNode::new("td").with_class("content");

    // The admonition's own title renders inside the content cell, before the
    // content itself.
    if let Some(title) = admonition.title() {
        content_cell
            .children
            .push(VirtualNode::new("div").with_class("title").with_text(title));
    }

    match admonition.content_model() {
        ContentModel::Compound => {
            for child in admonition.nested_blocks() {
                add_block_with_title(&mut content_cell, child);
            }
        }

        // Simple content is rendered directly in the content cell (no wrapping
        // paragraph), matching Asciidoctor's HTML output.
        _ => {
            if let Some(content) = admonition.content() {
                let rendered = content.rendered();
                if rendered.contains('<') {
                    content_cell.children.extend(parse_html_content(rendered));
                } else {
                    content_cell.text = Some(decode_html_entities(rendered));
                }
            }
        }
    }

    let row = VirtualNode::new("tr")
        .with_child(icon_cell)
        .with_child(content_cell);

    node.with_child(VirtualNode::new("table").with_child(row))
}

fn quote_to_node<'a>(quote: &'a QuoteBlock<'a>) -> VirtualNode {
    // The outer wrapper carries `quoteblock` or `verseblock`, then any roles.
    let block_class = format!("{}block", quote.type_().name());
    let mut node = VirtualNode::new("div").with_class(block_class);

    for role in quote.roles() {
        node = node.with_class(role);
    }

    if let Some(id) = quote.id() {
        node = node.with_id(id);
    }

    // A quote/verse block's title renders inside the wrapper, before the
    // content.
    if let Some(title) = quote.title() {
        node.children
            .push(VirtualNode::new("div").with_class("title").with_text(title));
    }

    match quote.type_() {
        // A verse renders its content verbatim inside `<pre class="content">`.
        QuoteType::Verse => {
            let rendered = quote
                .content()
                .map(|c| c.rendered().to_string())
                .unwrap_or_default();
            node.children.push(
                VirtualNode::new("pre")
                    .with_class("content")
                    .with_text(rendered),
            );
        }

        // A quote renders its content inside `<blockquote>`. Compound content
        // (a delimited or markdown-style quote) holds nested blocks; simple
        // content (a styled or quoted paragraph) holds inline text rendered
        // directly, without a wrapping paragraph.
        QuoteType::Quote => {
            let mut blockquote = VirtualNode::new("blockquote");

            match quote.content_model() {
                ContentModel::Compound => {
                    // `blocks()` (rather than `nested_blocks()`) is used so that
                    // a Markdown-style blockquote's nested blocks, which borrow
                    // the block's own owned source, are rendered too.
                    for child in quote.blocks() {
                        add_block_with_title(&mut blockquote, child);
                    }
                }

                _ => {
                    if let Some(content) = quote.content() {
                        let rendered = content.rendered();
                        if rendered.contains('<') {
                            blockquote.children.extend(parse_html_content(rendered));
                        } else {
                            blockquote.text = Some(decode_html_entities(rendered));
                        }
                    }
                }
            }

            node.children.push(blockquote);
        }
    }

    // The attribution (and optional citation) render in a trailing
    // `<div class="attribution">`, introduced by an em dash.
    if let Some(attribution_node) = quote_attribution_node(quote) {
        node.children.push(attribution_node);
    }

    node
}

/// Builds the `<div class="attribution">` for a quote or verse block, or `None`
/// when the block has neither an attribution nor a citation.
fn quote_attribution_node(quote: &QuoteBlock<'_>) -> Option<VirtualNode> {
    let attribution = quote.attribution();
    let citetitle = quote.citetitle();

    if attribution.is_none() && citetitle.is_none() {
        return None;
    }

    let mut node = VirtualNode::new("div").with_class("attribution");

    // The attribution text is introduced by an em dash. When only a citation is
    // present (no attribution), the em dash is omitted and the citation stands
    // alone.
    if let Some(attribution) = attribution {
        // The em dash is emitted as a numeric entity so the lead text stays
        // ASCII for `parse_html_content` (which is not multi-byte safe); it is
        // decoded back to `—` when the text node is built.
        let lead = format!("&#8212; {attribution}");
        node.children.extend(parse_html_content(&lead));
    }

    if let Some(citetitle) = citetitle {
        node.children
            .push(VirtualNode::new("cite").with_html_content(citetitle));
    }

    Some(node)
}

fn table_to_node<'a>(table: &'a TableBlock<'a>) -> VirtualNode {
    // The table-level classes mirror Asciidoctor's HTML backend: always
    // `tableblock`, then the frame and grid classes, then a width class.
    let mut classes = vec![
        "tableblock".to_string(),
        frame_class(table.frame()).to_string(),
        grid_class(table.grid()).to_string(),
    ];

    // A table whose columns are sized to their content renders `fit-content`; a
    // full-width (default) table renders `stretch`; a table with an explicit
    // width renders neither (the width travels in the `width` attribute).
    let autowidth = table.columns().iter().any(TableColumn::is_autowidth);
    if autowidth {
        classes.push("fit-content".to_string());
    } else if table.width().is_none() {
        classes.push("stretch".to_string());
    }

    if let Some(stripes) = stripes_class(table.stripes()) {
        classes.push(stripes.to_string());
    }

    // The `float` attribute renders as a bare direction class (e.g. `left`).
    if let Some(float) = table
        .attrlist()
        .and_then(|a| a.named_attribute("float"))
        .map(|a| a.value())
    {
        classes.push(float.to_string());
    }

    let mut node = VirtualNode::new("table").with_classes(classes);

    if let Some(id) = table.id() {
        node = node.with_id(id);
    }

    for role in table.roles() {
        node = node.with_class(role);
    }

    // An explicit table width is carried in the `width` attribute as a
    // percentage, matching `table[width="50%"]`-style assertions.
    if let Some(width) = table.width() {
        node = node.with_attribute("width", format!("{width}%"));
    }

    if let Some(title) = table.title() {
        // A titled table renders a <caption class="title">. When the processor
        // assigned an automatic caption (e.g. "Table 1. "), it is prepended to
        // the title text.
        let caption_text = match table.caption() {
            Some(caption) => format!("{caption}{title}"),
            None => title.to_string(),
        };

        node.children.push(
            VirtualNode::new("caption")
                .with_class("title")
                .with_text(caption_text),
        );
    }

    // A table with no rows at all (e.g. every row was dropped for exceeding the
    // column count) renders as a bare `<table>` with no colgroup or sections.
    if table.header_row().is_none() && table.body_rows().is_empty() && table.footer_row().is_none()
    {
        return node;
    }

    let mut colgroup = VirtualNode::new("colgroup");
    for (column, pcwidth) in table.columns().iter().zip(column_pcwidths(table.columns())) {
        // Every column exposes its computed percentage width in `colpcwidth`,
        // mirroring the model attribute Asciidoctor assigns to each column.
        let mut col = VirtualNode::new("col").with_attribute("colpcwidth", pcwidth.clone());

        if column.is_autowidth() {
            // An autowidth column is sized to its content: it carries the
            // `autowidth-option` marker (present with an empty value) and no HTML
            // `width` attribute.
            col = col.with_attribute("autowidth-option", "");
        } else {
            // A proportional column emits its percentage as the HTML `width`.
            col = col.with_attribute("width", format!("{pcwidth}%"));
        }

        colgroup.children.push(col);
    }
    node.children.push(colgroup);

    if let Some(header) = table.header_row() {
        let mut thead = VirtualNode::new("thead");
        thead.children.push(table_row_to_node(header, true, false));
        node.children.push(thead);
    }

    if !table.body_rows().is_empty() {
        let mut tbody = VirtualNode::new("tbody");
        for row in table.body_rows() {
            tbody.children.push(table_row_to_node(row, false, true));
        }
        node.children.push(tbody);
    }

    // The footer renders after the body, so the section order is
    // thead, tbody, tfoot.
    if let Some(footer) = table.footer_row() {
        let mut tfoot = VirtualNode::new("tfoot");
        tfoot.children.push(table_row_to_node(footer, false, true));
        node.children.push(tfoot);
    }

    node
}

/// Renders one table row. `header_row` marks the row as the table's header (its
/// cells become `<th>` and their content is not wrapped in a paragraph);
/// otherwise cells are `<td>` and body content is wrapped in a
/// `<p class="tableblock">` (`wrap_in_paragraph`).
fn table_row_to_node(row: &TableRow<'_>, header_row: bool, wrap_in_paragraph: bool) -> VirtualNode {
    let mut tr = VirtualNode::new("tr");

    for cell in row.cells() {
        // A cell carrying the header style renders as a `<th>` even outside the
        // header row; otherwise header rows use `<th>` and body/footer rows use
        // `<td>`.
        let cell_tag = if header_row || cell.style() == ColumnStyle::Header {
            "th"
        } else {
            "td"
        };

        let mut cell_node = VirtualNode::new(cell_tag).with_classes([
            "tableblock".to_string(),
            halign_class(cell.h_align()).to_string(),
            valign_class(cell.v_align()).to_string(),
        ]);

        if cell.colspan() > 1 {
            cell_node = cell_node.with_attribute("colspan", cell.colspan().to_string());
        }
        if cell.rowspan() > 1 {
            cell_node = cell_node.with_attribute("rowspan", cell.rowspan().to_string());
        }

        match cell.content() {
            TableCellContent::Simple(content) => {
                let rendered = content.rendered().to_string();

                match cell.style() {
                    // A literal cell renders its content verbatim inside a
                    // `<div class="literal"><pre>…</pre></div>`.
                    ColumnStyle::Literal => {
                        cell_node.children.push(
                            VirtualNode::new("div")
                                .with_class("literal")
                                .with_child(VirtualNode::new("pre").with_html_content(rendered)),
                        );
                    }

                    _ if !wrap_in_paragraph => {
                        // Header-row cells place their content directly in the
                        // cell, wrapped only by any style element.
                        match style_wrapper(cell.style()) {
                            Some(tag) => cell_node
                                .children
                                .push(VirtualNode::new(tag).with_html_content(rendered)),
                            None => cell_node = cell_node.with_html_content(rendered),
                        }
                    }

                    // An empty body cell renders as an empty `<td>` with no
                    // paragraph.
                    _ if rendered.is_empty() => {}

                    style => match style_wrapper(style) {
                        // A text-styled cell wraps its content in a
                        // `<p class="tableblock">` with an inner style element
                        // (`<strong>`, `<em>`, `<code>`).
                        Some(tag) => {
                            cell_node.children.push(
                                VirtualNode::new("p")
                                    .with_class("tableblock")
                                    .with_child(VirtualNode::new(tag).with_html_content(rendered)),
                            );
                        }

                        // A plain cell renders each blank-line-separated
                        // paragraph as its own `<p class="tableblock">`, matching
                        // Asciidoctor (the parser stores them as one cell with
                        // embedded blank lines).
                        None => {
                            for para in
                                split_cell_paragraphs(content.original().data(), content.rendered())
                            {
                                cell_node.children.push(
                                    VirtualNode::new("p")
                                        .with_class("tableblock")
                                        .with_html_content(para),
                                );
                            }
                        }
                    },
                }
            }

            // An AsciiDoc-styled cell holds a nested document, which renders into
            // a `<div class="content">` wrapper inside the cell. The blocks
            // render as they would at the top level of a document (e.g.
            // paragraphs wrapped in `<div class="paragraph">`), so the cell
            // reuses `add_block_with_title`.
            TableCellContent::AsciiDoc(cell) => {
                let mut content = VirtualNode::new("div").with_class("content");

                // The nested document's title renders as an `<h1>` when shown.
                if let Some(title) = cell.title() {
                    content
                        .children
                        .push(VirtualNode::new("h1").with_text(title));
                }

                // The cell is a standalone document with its own `toc`
                // configuration (not inherited from the parent). Resolve it the
                // same way as the top-level document.
                let toc_mode = cell.toc_mode();
                let toc_data = toc_mode.is_enabled().then(|| {
                    TocData::build(
                        cell.toc_levels(),
                        cell.toc_title(),
                        cell.toc_class(),
                        cell.blocks(),
                    )
                });

                if matches!(toc_mode, TocMode::Auto | TocMode::Left | TocMode::Right)
                    && let Some(data) = &toc_data
                {
                    content.children.push(toc_block("toc", false, data));
                }

                // Always (re)scope both stashes for the cell body so a `toc::[]`
                // inside the cell never picks up the parent document's TOC; only
                // a cell with its own `toc: macro`/`toc: preamble` provides one.
                let _macro_scope = scoped_toc(
                    &MACRO_TOC,
                    match toc_mode {
                        TocMode::Macro => toc_data.clone(),
                        _ => None,
                    },
                );
                let _preamble_scope = scoped_toc(
                    &PREAMBLE_TOC,
                    match toc_mode {
                        TocMode::Preamble => toc_data,
                        _ => None,
                    },
                );

                if cell.is_inline() {
                    // An `inline` doctype renders block content as bare inline
                    // content, without the `<div class="paragraph"><p>` wrapper.
                    for block in cell.blocks() {
                        match block.rendered_content() {
                            Some(rendered) => {
                                content.children.extend(parse_html_content(rendered));
                            }
                            None => add_block_with_title(&mut content, block),
                        }
                    }
                } else {
                    for block in cell.blocks() {
                        add_block_with_title(&mut content, block);
                    }
                }

                cell_node.children.push(content);
            }
        }

        tr.children.push(cell_node);
    }

    tr
}

/// Computes the `colpcwidth` (percentage width) that Asciidoctor assigns to
/// every column, mirroring `Table#assign_column_widths`.
///
/// Each autowidth column takes an equal share of the space the fixed columns
/// leave free (`(100 - fixed_total) / autowidth_count`); every column's
/// percentage is then truncated to four decimal places and the rounding balance
/// is donated to the final column so the widths sum to exactly 100. The value
/// is returned for every column (including autowidth columns, which carry the
/// percentage in the model even though the HTML backend omits their `width`
/// attribute).
fn column_pcwidths(columns: &[TableColumn]) -> Vec<String> {
    let n = columns.len();
    if n == 0 {
        return vec![];
    }

    let fixed_total: usize = columns
        .iter()
        .filter(|c| !c.is_autowidth())
        .map(TableColumn::width)
        .sum();

    // Resolve the effective per-column width and the base they are a percentage
    // of. With autowidth columns present the base is the full 100% and each
    // autowidth column takes an equal share of the remaining space (collapsing
    // to zero when the fixed columns already exceed 100%); otherwise each
    // column's own proportional width is taken over the sum of all widths.
    let (effective, base): (Vec<f64>, f64) = if columns.iter().any(TableColumn::is_autowidth) {
        let autowidth_count = columns.iter().filter(|c| c.is_autowidth()).count();
        let (share, base) = if fixed_total > 100 {
            (0.0, fixed_total as f64)
        } else {
            (
                truncate4((100.0 - fixed_total as f64) / autowidth_count as f64),
                100.0,
            )
        };
        let effective = columns
            .iter()
            .map(|c| {
                if c.is_autowidth() {
                    share
                } else {
                    c.width() as f64
                }
            })
            .collect();
        (effective, base)
    } else {
        let base = if fixed_total == 0 {
            n as f64
        } else {
            fixed_total as f64
        };
        (columns.iter().map(|c| c.width() as f64).collect(), base)
    };

    let mut pct: Vec<f64> = effective
        .iter()
        .map(|w| truncate4(w * 100.0 / base))
        .collect();

    // Donate the rounding balance to the final column.
    let total: f64 = pct.iter().sum();
    if (total - 100.0).abs() > 1e-9 {
        let last = n - 1;
        pct[last] = round4(100.0 - total + pct[last]);
    }

    pct.iter().map(|w| format_pcwidth(*w)).collect()
}

/// Splits a plain (non-styled) table cell's content into paragraphs, returning
/// the rendered text of each.
///
/// A paragraph break is a blank line in the **source**, not the rendered text.
/// This matters for an attribute reference such as `{blank}`: a line containing
/// only `{blank}` renders as an empty line but is not blank in the source, so
/// it must not split the paragraph (matching Asciidoctor). Inline substitution
/// is line-preserving, so the source and rendered line counts line up; each
/// non-blank source line contributes its rendered counterpart to the current
/// paragraph. If the two ever diverge in length, fall back to splitting the
/// rendered text on blank lines.
fn split_cell_paragraphs(source: &str, rendered: &str) -> Vec<String> {
    let source_lines: Vec<&str> = source.split('\n').collect();
    let rendered_lines: Vec<&str> = rendered.split('\n').collect();

    if source_lines.len() != rendered_lines.len() {
        return rendered
            .split("\n\n")
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
    }

    let mut paragraphs: Vec<String> = vec![];
    let mut current: Vec<&str> = vec![];
    for (src, rendered) in source_lines.iter().zip(rendered_lines.iter()) {
        if src.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n").trim().to_string());
                current.clear();
            }
        } else {
            current.push(rendered);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n").trim().to_string());
    }

    paragraphs.retain(|p| !p.is_empty());
    paragraphs
}

fn truncate4(x: f64) -> f64 {
    (x * 10000.0).trunc() / 10000.0
}

fn round4(x: f64) -> f64 {
    (x * 10000.0).round() / 10000.0
}

/// Formats a percentage width the way Asciidoctor serializes it: whole numbers
/// have no decimal part, and fractional values keep up to four significant
/// decimal places with trailing zeros trimmed (e.g. `50`, `17.647`, `33.3334`).
fn format_pcwidth(x: f64) -> String {
    let s = format!("{x:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

/// The inline element a text-styled cell wraps its content in, if any.
fn style_wrapper(style: ColumnStyle) -> Option<&'static str> {
    match style {
        ColumnStyle::Strong => Some("strong"),
        ColumnStyle::Emphasis => Some("em"),
        ColumnStyle::Monospace => Some("code"),
        _ => None,
    }
}

fn frame_class(frame: Frame) -> &'static str {
    match frame {
        Frame::All => "frame-all",
        Frame::Ends => "frame-ends",
        Frame::Sides => "frame-sides",
        Frame::None => "frame-none",
    }
}

fn grid_class(grid: Grid) -> &'static str {
    match grid {
        Grid::All => "grid-all",
        Grid::Rows => "grid-rows",
        Grid::Cols => "grid-cols",
        Grid::None => "grid-none",
    }
}

fn stripes_class(stripes: Stripes) -> Option<&'static str> {
    match stripes {
        Stripes::None => None,
        Stripes::Even => Some("stripes-even"),
        Stripes::Odd => Some("stripes-odd"),
        Stripes::All => Some("stripes-all"),
        Stripes::Hover => Some("stripes-hover"),
    }
}

fn halign_class(align: HorizontalAlignment) -> &'static str {
    match align {
        HorizontalAlignment::Left => "halign-left",
        HorizontalAlignment::Center => "halign-center",
        HorizontalAlignment::Right => "halign-right",
    }
}

fn valign_class(align: VerticalAlignment) -> &'static str {
    match align {
        VerticalAlignment::Top => "valign-top",
        VerticalAlignment::Middle => "valign-middle",
        VerticalAlignment::Bottom => "valign-bottom",
    }
}

fn preamble_to_node<'a>(preamble: &'a Preamble<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("div").with_id("preamble");

    for child in preamble.nested_blocks() {
        // A `toc::[]` macro in the preamble renders the table of contents.
        if is_toc_macro(child) {
            if let Some(toc) = toc_macro_node(child) {
                node.children.push(toc);
            }
            continue;
        }

        node.children.push(child.to_virtual_dom());
    }

    // A `toc: preamble` placement renders the table of contents immediately
    // below the preamble's content (inside the preamble container). Taking it
    // ensures it renders only once, even though there is only ever one
    // preamble.
    if let Some(data) = PREAMBLE_TOC.with(|p| p.borrow_mut().take()) {
        node.children.push(toc_block("toc", false, &data));
    }

    node
}

fn break_to_node<'a>(break_: &'a Break<'a>) -> VirtualNode {
    let context = break_.raw_context();

    match context.as_ref() {
        "thematic_break" => VirtualNode::new("hr"),
        "page_break" => VirtualNode::new("div").with_class("page-break"),
        _ => VirtualNode::new("hr"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::prelude::*;

    #[test]
    fn empty_document() {
        let doc = Parser::default().parse("");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.tag, "div");
        assert_eq!(vdom.classes, vec!["document"]);
        assert_eq!(vdom.children.len(), 0);
    }

    #[test]
    fn single_paragraph() {
        let doc = Parser::default().parse("Hello, world!");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.tag, "div");
        assert_eq!(vdom.classes, vec!["document"]);
        assert_eq!(vdom.children.len(), 1);

        // Top-level paragraphs are wrapped in div.paragraph.
        let wrapper = &vdom.children[0];
        assert_eq!(wrapper.tag, "div");
        assert!(wrapper.classes.contains(&"paragraph".to_string()));
        assert_eq!(wrapper.children.len(), 1);

        let para = &wrapper.children[0];
        assert_eq!(para.tag, "p");
        assert_eq!(para.text.as_deref(), Some("Hello, world!"));
    }

    #[test]
    fn unordered_list() {
        let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.children.len(), 1);

        let wrapper = &vdom.children[0];
        assert_eq!(wrapper.tag, "div");
        assert!(wrapper.classes.contains(&"ulist".to_string()));
        assert_eq!(wrapper.children.len(), 1);

        let ul = &wrapper.children[0];
        assert_eq!(ul.tag, "ul");
        assert_eq!(ul.children.len(), 3);

        for li in &ul.children {
            assert_eq!(li.tag, "li");
        }
    }

    #[test]
    fn section_with_paragraph() {
        let doc = Parser::default().parse("== Section Title\n\nSome text.");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.children.len(), 1);

        let section = &vdom.children[0];
        assert_eq!(section.tag, "div");
        assert!(section.classes.contains(&"sect1".to_string()));

        // Section contains heading and paragraph wrapper.
        assert_eq!(section.children.len(), 2);
        assert_eq!(section.children[0].tag, "h2");

        // Paragraph is wrapped in div.paragraph.
        let para_wrapper = &section.children[1];
        assert_eq!(para_wrapper.tag, "div");
        assert!(para_wrapper.classes.contains(&"paragraph".to_string()));
        assert_eq!(para_wrapper.children.len(), 1);
        assert_eq!(para_wrapper.children[0].tag, "p");
    }

    #[test]
    fn ordered_list_has_arabic_class() {
        let doc = Parser::default().parse(". item 1\n. item 2\n. item 3");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.children.len(), 1);

        let wrapper = &vdom.children[0];
        assert_eq!(wrapper.tag, "div");
        assert!(wrapper.classes.contains(&"olist".to_string()));
        // Wrapper should also have "arabic" class for ordered lists.
        assert!(wrapper.classes.contains(&"arabic".to_string()));
        assert_eq!(wrapper.children.len(), 1);

        let ol = &wrapper.children[0];
        assert_eq!(ol.tag, "ol");
        // The <ol> element should also have "arabic" class.
        assert!(ol.classes.contains(&"arabic".to_string()));
        assert_eq!(ol.children.len(), 3);

        for li in &ol.children {
            assert_eq!(li.tag, "li");
        }
    }

    #[test]
    fn inline_html_markup_in_paragraph() {
        let doc = Parser::default().parse("I am *strong* and _emphasized_ and `code`.");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.children.len(), 1);

        // Top-level paragraphs are wrapped in div.paragraph.
        let wrapper = &vdom.children[0];
        assert_eq!(wrapper.tag, "div");
        assert!(wrapper.classes.contains(&"paragraph".to_string()));
        assert_eq!(wrapper.children.len(), 1);

        let para = &wrapper.children[0];
        assert_eq!(para.tag, "p");

        // Should have parsed HTML into child nodes.
        assert!(
            !para.children.is_empty(),
            "Should have child nodes from parsed HTML"
        );

        // Verify strong element.
        let strong = para.children.iter().find(|c| c.tag == "strong");
        assert!(strong.is_some(), "Should have a <strong> element");
        assert_eq!(strong.unwrap().text.as_deref(), Some("strong"));

        // Verify em element.
        let em = para.children.iter().find(|c| c.tag == "em");
        assert!(em.is_some(), "Should have an <em> element");
        assert_eq!(em.unwrap().text.as_deref(), Some("emphasized"));

        // Verify code element.
        let code = para.children.iter().find(|c| c.tag == "code");
        assert!(code.is_some(), "Should have a <code> element");
        assert_eq!(code.unwrap().text.as_deref(), Some("code"));
    }

    #[test]
    fn titled_table_renders_captioned_title() {
        let doc = Parser::default().parse(".A table with a title\n|===\n|a |b\n|===");
        let vdom = doc.to_virtual_dom();

        let table = &vdom.children[0];
        assert_eq!(table.tag, "table");

        let caption = &table.children[0];
        assert_eq!(caption.tag, "caption");
        assert!(caption.classes.contains(&"title".to_string()));
        assert_eq!(
            caption.text.as_deref(),
            Some("Table 1. A table with a title")
        );
    }

    #[test]
    fn description_list_uses_dt_and_dd_tags() {
        let doc = Parser::default().parse("term1:: definition1\nterm2:: definition2");
        let vdom = doc.to_virtual_dom();

        assert_eq!(vdom.children.len(), 1);

        let wrapper = &vdom.children[0];
        assert_eq!(wrapper.tag, "div");
        assert!(wrapper.classes.contains(&"dlist".to_string()));
        assert_eq!(wrapper.children.len(), 1);

        let dl = &wrapper.children[0];
        assert_eq!(dl.tag, "dl");
        // Should have 4 children: dt, dd, dt, dd.
        assert_eq!(dl.children.len(), 4);

        // Check first term/definition pair.
        assert_eq!(dl.children[0].tag, "dt");
        assert_eq!(dl.children[0].text.as_deref(), Some("term1"));
        assert_eq!(dl.children[1].tag, "dd");
        assert_eq!(dl.children[1].children.len(), 1);
        assert_eq!(dl.children[1].children[0].tag, "p");
        assert_eq!(
            dl.children[1].children[0].text.as_deref(),
            Some("definition1")
        );

        // Check second term/definition pair.
        assert_eq!(dl.children[2].tag, "dt");
        assert_eq!(dl.children[2].text.as_deref(), Some("term2"));
        assert_eq!(dl.children[3].tag, "dd");
        assert_eq!(dl.children[3].children.len(), 1);
        assert_eq!(dl.children[3].children[0].tag, "p");
        assert_eq!(
            dl.children[3].children[0].text.as_deref(),
            Some("definition2")
        );
    }

    mod toc {
        use crate::{document::TocMode, tests::prelude::*};

        #[test]
        fn no_toc_when_attribute_absent() {
            let doc = Parser::default().parse("= Title\n\n== Section\n\ncontent");

            assert_eq!(doc.toc_mode(), TocMode::Disabled);
            assert_css(&doc, ".toc", 0);
        }

        #[test]
        fn auto_toc_renders_at_top_of_body() {
            let doc =
                Parser::default().parse("= Title\n:toc:\n\n== First\n\nhi\n\n== Second\n\nbye");

            assert_eq!(doc.toc_mode(), TocMode::Auto);

            // Exactly one TOC, carrying both `id="toc"` and `class="toc"`.
            assert_css(&doc, "#toc", 1);
            assert_css(&doc, ".toc", 1);

            // It is the first child of the document, ahead of the sections.
            let vdom = doc.to_virtual_dom();
            let toc = &vdom.children[0];
            assert_eq!(toc.tag, "div");
            assert_eq!(toc.id.as_deref(), Some("toc"));
            assert!(toc.classes.contains(&"toc".to_string()));

            // The title element has no `class="title"` for an automatic TOC.
            let title = &toc.children[0];
            assert_eq!(title.id.as_deref(), Some("toctitle"));
            assert_eq!(title.text.as_deref(), Some("Table of Contents"));
            assert!(title.classes.is_empty());

            // One `sectlevel1` entry per top-level section, each linking to it.
            assert_css(&doc, "ul.sectlevel1 > li > a", 2);
            let links = &toc.children[1];
            assert_eq!(links.tag, "ul");
            assert!(links.classes.contains(&"sectlevel1".to_string()));
            assert_eq!(
                links.children[0].children[0].attributes.get("href"),
                Some(&"#_first".to_string())
            );
            assert_eq!(links.children[0].children[0].text.as_deref(), Some("First"));
        }

        #[test]
        fn empty_toc_resolves_to_auto() {
            // An explicit `:toc: auto` and an empty `:toc:` both resolve to the
            // automatic top-of-body placement.
            for value in ["", " auto"] {
                let doc =
                    Parser::default().parse(&format!("= Title\n:toc:{value}\n\n== Section\n\nhi"));

                assert_eq!(doc.toc_mode(), TocMode::Auto, "value: {value:?}");
                let vdom = doc.to_virtual_dom();
                assert_eq!(vdom.children[0].id.as_deref(), Some("toc"));
            }
        }

        #[test]
        fn left_and_right_render_like_auto_at_top() {
            // `left`/`right` request a side column in standalone HTML, but this
            // embeddable virtual DOM (Asciidoctor's documented fallback) renders
            // them like `auto`: a `div#toc` at the top of the body.
            for (value, mode) in [("left", TocMode::Left), ("right", TocMode::Right)] {
                let doc =
                    Parser::default().parse(&format!("= Title\n:toc: {value}\n\n== Section\n\nhi"));

                assert_eq!(doc.toc_mode(), mode, "value: {value}");
                assert_css(&doc, "#toc.toc", 1);

                let vdom = doc.to_virtual_dom();
                assert_eq!(vdom.children[0].id.as_deref(), Some("toc"));
            }
        }

        #[test]
        fn preamble_toc_renders_below_preamble() {
            let doc = Parser::default()
                .parse("= Title\n:toc: preamble\n\nintro para\n\n== Section\n\nhi");

            assert_eq!(doc.toc_mode(), TocMode::Preamble);

            // Exactly one TOC, and it lives inside the preamble (not at the top
            // of the body), as the last child after the preamble's content.
            assert_css(&doc, ".toc", 1);
            assert_css(&doc, "#preamble > #toc.toc", 1);

            let vdom = doc.to_virtual_dom();
            assert_ne!(vdom.children[0].id.as_deref(), Some("toc"));

            let preamble = vdom
                .children
                .iter()
                .find(|c| c.id.as_deref() == Some("preamble"))
                .expect("preamble present");
            assert_eq!(
                preamble.children.last().and_then(|c| c.id.as_deref()),
                Some("toc")
            );

            // A preamble TOC's title carries no `class="title"` (like `auto`).
            assert_css(&doc, "#toctitle.title", 0);
        }

        #[test]
        fn preamble_toc_absent_without_preamble() {
            // The CAUTION on the position page: a `toc: preamble` placement does
            // not appear when the document has no preamble.
            let doc = Parser::default().parse("= Title\n:toc: preamble\n\n== Section\n\nhi");

            assert_eq!(doc.toc_mode(), TocMode::Preamble);
            assert_css(&doc, ".toc", 0);
        }

        #[test]
        fn auto_toc_nesting_honors_default_toclevels() {
            let doc = Parser::default()
                .parse("= Title\n:toc:\n\n== Level 1\n\n=== Level 2\n\n==== Level 3\n\ncontent");

            // The default `toclevels` is 2, so the `==` and `===` sections appear
            // but the `====` (level 3) section is excluded.
            assert_eq!(doc.toc_levels(), 2);
            assert_css(&doc, "ul.sectlevel1", 1);
            assert_css(&doc, "ul.sectlevel2", 1);
            assert_css(&doc, "ul.sectlevel3", 0);
        }

        #[test]
        fn toclevels_controls_toc_depth() {
            let doc = Parser::default().parse(
                "= Title\n:toc:\n:toclevels: 3\n\n== L1\n\n=== L2\n\n==== L3\n\n===== L4\n\nx",
            );

            // `toclevels: 3` includes `==`, `===`, and `====`, but not `=====`.
            assert_eq!(doc.toc_levels(), 3);
            assert_css(&doc, "ul.sectlevel1", 1);
            assert_css(&doc, "ul.sectlevel2", 1);
            assert_css(&doc, "ul.sectlevel3", 1);
            assert_css(&doc, "ul.sectlevel4", 0);
        }

        #[test]
        fn toclevels_zero_is_coerced_to_one() {
            // With no parts, `toclevels: 0` is coerced to `1`.
            let doc =
                Parser::default().parse("= Title\n:toc:\n:toclevels: 0\n\n== L1\n\n=== L2\n\nx");

            assert_eq!(doc.toc_levels(), 1);
            assert_css(&doc, "ul.sectlevel1", 1);
            assert_css(&doc, "ul.sectlevel2", 0);
        }

        #[test]
        fn invalid_toclevels_falls_back_to_default() {
            let doc = Parser::default()
                .parse("= Title\n:toc:\n:toclevels: huge\n\n== L1\n\n=== L2\n\n==== L3\n\nx");

            assert_eq!(doc.toc_levels(), 2);
            assert_css(&doc, "ul.sectlevel2", 1);
            assert_css(&doc, "ul.sectlevel3", 0);
        }

        #[test]
        fn custom_toc_title() {
            let doc = Parser::default()
                .parse("= Title\n:toc:\n:toc-title: Table of Adventures\n\n== Section\n\nhi");

            assert_eq!(doc.toc_title(), "Table of Adventures");
            let vdom = doc.to_virtual_dom();
            let title = &vdom.children[0].children[0];
            assert_eq!(title.id.as_deref(), Some("toctitle"));
            assert_eq!(title.text.as_deref(), Some("Table of Adventures"));
        }

        #[test]
        fn empty_toc_title_renders_empty() {
            // An empty `:toc-title:` yields an empty title heading.
            let doc = Parser::default().parse("= Title\n:toc:\n:toc-title:\n\n== Section\n\nhi");

            assert_eq!(doc.toc_title(), "");
            let vdom = doc.to_virtual_dom();
            assert_eq!(vdom.children[0].children[0].text.as_deref(), Some(""));
        }

        #[test]
        fn custom_toc_class() {
            let doc = Parser::default()
                .parse("= Title\n:toc:\n:toc-class: floating-toc\n\n== Section\n\nhi");

            assert_eq!(doc.toc_class(), "floating-toc");
            // The container carries the custom class in place of the default.
            assert_css(&doc, "#toc.floating-toc", 1);
            assert_css(&doc, "#toc.toc", 0);
        }

        #[test]
        fn toc_defaults_when_attributes_unset() {
            // With `toc` enabled but the others unset, the resolved depth, title,
            // and class are the documented defaults.
            let doc = Parser::default().parse("= Title\n:toc:\n\n== Section\n\nhi");

            assert_eq!(doc.toc_levels(), 2);
            assert_eq!(doc.toc_title(), "Table of Contents");
            assert_eq!(doc.toc_class(), "toc");
        }

        #[test]
        fn macro_toc_renders_at_macro_not_at_top() {
            let doc = Parser::default()
                .parse("= Title\n:toc: macro\n\npreamble\n\ntoc::[]\n\n== Section\n\nhi");

            assert_eq!(doc.toc_mode(), TocMode::Macro);
            assert_css(&doc, ".toc", 1);

            // The TOC is *not* the first child of the document body (it renders
            // at the macro's location, inside the preamble).
            let vdom = doc.to_virtual_dom();
            assert_ne!(vdom.children[0].id.as_deref(), Some("toc"));
            assert_css(&doc, "#preamble .toc", 1);

            // A macro TOC's title element carries `class="title"`.
            assert_css(&doc, "#toctitle.title", 1);
        }

        #[test]
        fn macro_toc_uses_explicit_id() {
            let doc = Parser::default()
                .parse("= Title\n:toc: macro\n\n[#my-toc]\ntoc::[]\n\n== Section\n\nhi");

            assert_css(&doc, "#my-toc.toc", 1);
            assert_css(&doc, "#my-toctitle.title", 1);
        }

        #[test]
        fn macro_placement_without_macro_renders_no_toc() {
            // `toc: macro` only renders where a `toc::[]` macro appears; with no
            // such macro, no TOC is generated.
            let doc = Parser::default().parse("= Title\n:toc: macro\n\n== Section\n\nhi");

            assert_eq!(doc.toc_mode(), TocMode::Macro);
            assert_css(&doc, ".toc", 0);
        }

        #[test]
        fn toc_macro_renders_nothing_when_toc_not_enabled() {
            // A stray `toc::[]` with no `toc` attribute renders nothing — neither
            // a TOC nor the literal paragraph text.
            let doc = Parser::default().parse("= Title\n\ntoc::[]\n\n== Section\n\nhi");

            assert_eq!(doc.toc_mode(), TocMode::Disabled);
            assert_css(&doc, ".toc", 0);

            let dom_text = to_dom_text(&doc);
            assert!(
                !dom_text.contains("toc::"),
                "unexpected literal macro text in: {dom_text}"
            );
        }

        /// Collects all text content in the rendered virtual DOM.
        fn to_dom_text(doc: &crate::Document) -> String {
            fn collect(node: &super::super::VirtualNode, out: &mut String) {
                if let Some(text) = &node.text {
                    out.push_str(text);
                }
                for child in &node.children {
                    collect(child, out);
                }
            }

            let mut out = String::new();
            collect(&doc.to_virtual_dom(), &mut out);
            out
        }
    }
}
