//! Virtual DOM representation of parsed AsciiDoc documents.
//!
//! This module provides a lightweight HTML-like representation of AsciiDoc
//! documents for testing purposes. It maps AsciiDoc block structures to their
//! HTML equivalents, enabling XPath-like queries for test assertions.

use crate::{
    Document, HasSpan,
    blocks::{
        Block, Break, CompoundDelimitedBlock, IsBlock, ListBlock, ListItem, ListItemMarker,
        ListType, MediaBlock, Preamble, RawDelimitedBlock, SectionBlock, SimpleBlock,
        SimpleBlockStyle,
    },
};

/// Decodes common HTML entities to their character equivalents.
///
/// This simulates what a browser would do when parsing HTML and accessing
/// text content via JavaScript's `textContent` or XPath's `text()`.
fn decode_html_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
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

    // Check for self-closing tag.
    if tag_content.ends_with('/') {
        return None; // Ignore self-closing tags.
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

    Some((element, after_closing))
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
        let mut node = VirtualNode::new("div").with_class("document");

        // Add document ID if present.
        if let Some(id) = self.id() {
            node = node.with_id(id);
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
    // Check if this block type handles its own title internally.
    let handles_title_internally = matches!(block, Block::List(_));

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
    if !is_horizontal
        && let Some(style) = list.declared_style() {
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

    // For ordered lists, add style class to the wrapper based on marker length,
    // but only if no explicit style is declared.
    if list.type_() == ListType::Ordered
        && list.declared_style().is_none()
        && let Some(style) = list.marker_style()
    {
        wrapper = wrapper.with_class(style);
    }

    // Add style class to the wrapper if present (explicit style overrides marker
    // style). Skip for horizontal dlists since wrapper already has hdlist class.
    if !is_horizontal
        && let Some(style) = list.declared_style() {
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

            // Add the block's rendered content to the code element.
            if let Some(content) = raw.rendered_content() {
                code = code.with_text(content);
            }

            let pre = VirtualNode::new("pre").with_child(code);
            node.children.push(pre);
        } else {
            let mut pre = VirtualNode::new("pre");
            if let Some(content) = raw.rendered_content() {
                pre = pre.with_text(content);
            }
            node.children.push(pre);
        }
    }

    node
}

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

    for child in compound.nested_blocks() {
        node.children.push(child.to_virtual_dom());
    }

    node
}

fn preamble_to_node<'a>(preamble: &'a Preamble<'a>) -> VirtualNode {
    let mut node = VirtualNode::new("div").with_id("preamble");

    for child in preamble.nested_blocks() {
        node.children.push(child.to_virtual_dom());
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
    use crate::Parser;

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
}
