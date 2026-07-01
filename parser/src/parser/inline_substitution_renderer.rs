use std::{fmt::Debug, sync::LazyLock};

use regex::Regex;

use crate::{Parser, attributes::Attrlist, parser::ResolvedReference};

/// An implementation of `InlineSubstitutionRenderer` is used when converting
/// the basic raw text of a simple block to the format which will ultimately be
/// presented in the final converted output.
///
/// An implementation is provided for HTML output; alternative implementations
/// (not provided in this crate) could support other output formats.
pub trait InlineSubstitutionRenderer: Debug {
    /// Renders the substitution for a special character.
    ///
    /// The renderer should write the appropriate rendering to `dest`.
    fn render_special_character(&self, type_: SpecialCharacter, dest: &mut String);

    /// Renders the content of a [quote substitution].
    ///
    /// The renderer should write the appropriate rendering to `dest`.
    ///
    /// [quote substitution]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
    fn render_quoted_substitition(
        &self,
        type_: QuoteType,
        scope: QuoteScope,
        attrlist: Option<Attrlist<'_>>,
        id: Option<String>,
        body: &str,
        dest: &mut String,
    );

    /// Renders the content of a [character replacement].
    ///
    /// The renderer should write the appropriate rendering to `dest`.
    ///
    /// [character replacement]: https://docs.asciidoctor.org/asciidoc/latest/subs/replacements/
    fn render_character_replacement(&self, type_: CharacterReplacementType, dest: &mut String);

    /// Renders a line break.
    ///
    /// The renderer should write an appropriate rendering of line break to
    /// `dest`.
    ///
    /// This is used in the implementation of [post-replacement substitutions].
    ///
    /// [post-replacement substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/post-replacements/
    fn render_line_break(&self, dest: &mut String);

    /// Renders an image.
    ///
    /// The renderer should write an appropriate rendering of the specified
    /// image to `dest`.
    fn render_image(&self, params: &ImageRenderParams, dest: &mut String);

    /// Construct a URI reference or data URI to the target image.
    ///
    /// If the `target_image_path` is a URI reference, then leave it untouched.
    ///
    /// The `target_image_path` is resolved relative to the directory retrieved
    /// from the specified document-scoped attribute key, if provided.
    ///
    /// NOT YET IMPLEMENTED:
    /// If the `data-uri` attribute is set on the document, and the safe mode
    /// level is less than `SafeMode::SECURE`, the image will be safely
    /// converted to a data URI by reading it from the same directory. If
    /// neither of these conditions are satisfied, a relative path (i.e., URL)
    /// will be returned.
    ///
    /// ## Parameters
    ///
    /// * `target_image_path`: path to the target image
    /// * `parser`: Current document parser state
    /// * `asset_dir_key`: If provided, the attribute key used to look up the
    ///   directory where the image is located. If not provided, `imagesdir` is
    ///   used.
    ///
    /// ## Return
    ///
    /// Returns a string reference or data URI for the target image that can be
    /// safely used in an image tag.
    fn image_uri(
        &self,
        target_image_path: &str,
        parser: &Parser,
        asset_dir_key: Option<&str>,
    ) -> String;

    /// Renders an icon.
    ///
    /// The renderer should write an appropriate rendering of the specified
    /// icon to `dest`.
    fn render_icon(&self, params: &IconRenderParams, dest: &mut String);

    /// Construct a reference or data URI to an icon image for the specified
    /// icon name.
    ///
    /// If the `icon` attribute is set on this block, the name is ignored and
    /// the value of this attribute is used as the target image path. Otherwise,
    /// construct a target image path by concatenating the value of the
    /// `iconsdir` attribute, the icon name, and the value of the `icontype`
    /// attribute (defaulting to `png`).
    ///
    /// The target image path is then passed through the `image_uri()` method.
    /// If the `data-uri` attribute is set on the document, the image will be
    /// safely converted to a data URI.
    ///
    /// The return value of this method can be safely used in an image tag.
    fn icon_uri(&self, name: &str, _attrlist: &Attrlist, parser: &Parser) -> String {
        let icontype = parser
            .attribute_value("icontype")
            .as_maybe_str()
            .unwrap_or("png")
            .to_owned();

        if false {
            todo!(
                "Enable this when doing block-related icon attributes: {}",
                r#"
                let icon = if let Some(icon) = attrlist.named_attribute("icon") {
                    let icon_str = icon.value();
                    if has_extname(icon_str) {
                        icon_str.to_string()
                    } else {
                        format!("{icon_str}.{icontype}")
                    }
                } else {
                    // This part is defaulted for now.
                    format!("{name}.{icontype}")
                };
            "#
            );
        }

        let icon = format!("{name}.{icontype}");

        self.image_uri(&icon, parser, Some("iconsdir"))
    }

    /// Renders a link.
    ///
    /// The renderer should write an appropriate rendering of the specified
    /// link, to `dest`.
    fn render_link(&self, params: &LinkRenderParams, dest: &mut String);

    /// Renders an anchor.
    ///
    /// The rendered should write an appropriate rendering of the specified
    /// anchor with ID and possible ref text (only used by some renderers).
    fn render_anchor(&self, id: &str, reftext: Option<String>, dest: &mut String);

    /// Renders a cross-reference.
    ///
    /// When [`XrefRenderParams::resolved`] is `Some`, the reference resolved to
    /// a destination; the renderer should link to it. When it is `None`, the
    /// reference could not be resolved and the renderer should emit a sensible
    /// fallback (e.g. a link to the raw target with bracketed text).
    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String);

    /// Renders a [callout] number that annotates a line in a verbatim block.
    ///
    /// The renderer should write an appropriate rendering of the callout number
    /// to `dest`. The rendering typically depends on whether font-based or
    /// image-based icons are enabled (via the `icons` document attribute).
    ///
    /// [callout]: https://docs.asciidoctor.org/asciidoc/latest/verbatim/callouts/
    fn render_callout(&self, params: &CalloutRenderParams, dest: &mut String);

    /// Renders an [index term].
    ///
    /// A *flow* (visible) index term ([`IndexTermRenderParams::visible_term`]
    /// is `Some`) appears in the flow of text, so the renderer should write
    /// the term text to `dest`. A *concealed* index term ([`visible_term`]
    /// is `None`) does not appear in the rendered text, so the renderer
    /// should typically write nothing.
    ///
    /// Note that the built-in HTML5 converter never builds an index catalog;
    /// index terms only contribute markup in output formats (such as DocBook or
    /// PDF) that generate an index.
    ///
    /// [index term]: https://docs.asciidoctor.org/asciidoc/latest/sections/user-index/
    /// [`visible_term`]: IndexTermRenderParams::visible_term
    fn render_index_term(&self, params: &IndexTermRenderParams, dest: &mut String);

    /// Renders the inline reference produced by a [`footnote`] macro.
    ///
    /// The footnote's *text* is not rendered here (it is extracted to the
    /// document's footnote list); this method renders only the superscript
    /// marker that appears in the flow of text and links to the footnote.
    ///
    /// See [`FootnoteRenderParams`] for the three cases the renderer must
    /// handle (a defining occurrence, a reference to an earlier footnote, and
    /// an unresolved reference).
    ///
    /// [`footnote`]: https://docs.asciidoctor.org/asciidoc/latest/macros/footnote/
    fn render_footnote(&self, params: &FootnoteRenderParams, dest: &mut String);
}

/// Specifies which special character is being replaced in a call to
/// [`InlineSubstitutionRenderer::render_special_character`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialCharacter {
    /// Replace `<` character.
    Lt,

    /// Replace `>` character.
    Gt,

    /// Replace `&` character.
    Ampersand,
}

/// Specifies which [quote type] is being rendered.
///
/// [quote type]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteType {
    /// Strong (often bold) formatting.
    Strong,

    /// Word(s) surrounded by smart double quotes.
    DoubleQuote,

    /// Word(s) surrounded by smart single quotes.
    SingleQuote,

    /// Monospace (code) formatting.
    Monospaced,

    /// Emphasis (often italic) formatting.
    Emphasis,

    /// Text range (span) formatted with zero or more styles.
    Mark,

    /// Superscript formatting.
    Superscript,

    /// Subscript formatting.
    Subscript,

    /// Surrounds a block of text that may need a `<span>` or similar tag.
    Unquoted,

    /// Inline AsciiMath expression, surrounded by AsciiMath math delimiters.
    AsciiMath,

    /// Inline LaTeX math expression, surrounded by LaTeX inline math
    /// delimiters.
    LatexMath,
}

/// Specifies whether the block is aligned to word boundaries or not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteScope {
    /// The quoted section was aligned to word boundaries.
    Constrained,

    /// The quoted section may not have been aligned to word boundaries.
    Unconstrained,
}

/// Specifies which [character replacement] is being rendered.
///
/// [character replacement]: https://docs.asciidoctor.org/asciidoc/latest/subs/replacements/
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterReplacementType {
    /// Copyright `(C)`.
    Copyright,

    /// Registered `(R)`.
    Registered,

    /// Trademark `(TM)`.
    Trademark,

    /// Em-dash surrounded by spaces ` -- `.
    EmDashSurroundedBySpaces,

    /// Em-dash without space `--`.
    EmDashWithoutSpace,

    /// Ellipsis `...`.
    Ellipsis,

    /// Single right arrow `->`.
    SingleRightArrow,

    /// Double right arrow `=>`.
    DoubleRightArrow,

    /// Single left arrow `<-`.
    SingleLeftArrow,

    /// Double left arrow `<=`.
    DoubleLeftArrow,

    /// Typographic apostrophe `'` within a word.
    TypographicApostrophe,

    /// Character reference `&___;`.
    CharacterReference(String),
}

/// Provides parsed parameters for an image to be rendered.
#[derive(Clone, Debug)]
pub struct ImageRenderParams<'a> {
    /// Target (the reference to the image).
    pub target: &'a str,

    /// Alt text (either explicitly set or defaulted).
    pub alt: String,

    /// Width. The data type is not checked; this may be any string.
    pub width: Option<&'a str>,

    /// Height. The data type is not checked; this may be any string.
    pub height: Option<&'a str>,

    /// Attribute list.
    pub attrlist: &'a Attrlist<'a>,

    /// Parser. The rendered may find document settings (such as an image
    /// directory) in the parser's document attributes.
    pub parser: &'a Parser,
}

/// Provides parsed parameters for an icon to be rendered.
#[derive(Clone, Debug)]
pub struct IconRenderParams<'a> {
    /// Target (the reference to the image).
    pub target: &'a str,

    /// Alt text (either explicitly set or defaulted).
    pub alt: String,

    /// Size. The data type is not checked; this may be any string.
    pub size: Option<&'a str>,

    /// Attribute list.
    pub attrlist: &'a Attrlist<'a>,

    /// Parser. The rendered may find document settings (such as an image
    /// directory) in the parser's document attributes.
    pub parser: &'a Parser,
}

/// Provides parsed parameters for an icon to be rendered.
#[derive(Clone, Debug)]
pub struct LinkRenderParams<'a> {
    /// Target (the target of this link).
    pub target: String,

    /// Link text.
    pub link_text: String,

    /// Roles (CSS classes) for this link not specified in the attrlist.
    pub extra_roles: Vec<&'a str>,

    /// Target window selection (passed through to `window` function in HTML).
    pub window: Option<&'static str>,

    /// What type of link is being rendered?
    pub type_: LinkRenderType,

    /// Attribute list.
    pub attrlist: &'a Attrlist<'a>,

    /// Parser. The rendered may find document settings (such as an image
    /// directory) in the parser's document attributes.
    pub parser: &'a Parser,
}

/// What type of link is being rendered?
#[derive(Clone, Debug)]
pub enum LinkRenderType {
    /// TEMPORARY: I don't know the different types of links yet.
    Link,
}

/// Provides parameters for rendering a [callout] number.
///
/// [callout]: https://docs.asciidoctor.org/asciidoc/latest/verbatim/callouts/
#[derive(Clone, Debug)]
pub struct CalloutRenderParams<'a> {
    /// The callout number to display. For automatically-numbered callouts
    /// (`<.>`), this is the resolved sequential number.
    pub number: &'a str,

    /// The guard surrounding the callout in the source. This controls whether
    /// (and how) the line-comment or XML-comment characters that hide the
    /// callout in the raw source are preserved in the output when icons are not
    /// enabled.
    pub guard: CalloutGuard<'a>,

    /// Parser. The renderer reads the `icons`, `iconsdir`, and `icontype`
    /// document attributes to decide how to render the callout.
    pub parser: &'a Parser,
}

/// Describes the characters that guard (hide) a callout number in verbatim
/// source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalloutGuard<'a> {
    /// A line-comment (or absent) guard. Holds the line-comment prefix that
    /// precedes the callout in the source (e.g. `# `), or an empty string when
    /// the callout is not tucked behind a line comment. When icons are not
    /// enabled, the prefix is preserved ahead of the rendered callout number.
    LineComment(&'a str),

    /// An XML comment guard (`<!--N-->`). When icons are not enabled, the XML
    /// comment delimiters are preserved around the rendered callout number.
    Xml,
}

/// Provides parameters for rendering a cross-reference.
#[derive(Clone, Debug)]
pub struct XrefRenderParams<'a> {
    /// The raw, uninterpreted cross-reference target as written in the source.
    pub target: &'a str,

    /// Explicit link text supplied in the cross-reference, if any.
    pub provided_text: Option<&'a str>,

    /// The resolved destination, or `None` if the reference is unresolved.
    pub resolved: Option<&'a ResolvedReference>,
}

/// Provides parameters for rendering an [index term].
///
/// [index term]: https://docs.asciidoctor.org/asciidoc/latest/sections/user-index/
#[derive(Clone, Debug)]
pub struct IndexTermRenderParams<'a> {
    /// For a *flow* (visible) index term (`((term))` or `indexterm2:[term]`),
    /// the already-substituted primary term text to display in the flow of
    /// text. `None` for a *concealed* index term (`(((p, s, t)))` or
    /// `indexterm:[p, s, t]`), which produces no visible output.
    pub visible_term: Option<&'a str>,
}

/// Provides parameters for rendering the inline marker of a [`footnote`] macro.
///
/// There are three cases the renderer must distinguish:
///
/// * A *defining* occurrence (`index` is `Some`, `is_reference` is `false`):
///   the footnote introduces new text. The marker carries the footnote number
///   and, when the footnote was given an ID, an `id` of its own.
/// * A *reference* to an earlier footnote (`index` is `Some`, `is_reference` is
///   `true`): a later occurrence (`footnote:id[]`) that reuses an existing
///   footnote's number.
/// * An *unresolved* reference (`index` is `None`, `is_reference` is `true`): a
///   reference whose ID was never defined; the renderer emits a visible error
///   marker built from [`text`](Self::text).
///
/// [`footnote`]: https://docs.asciidoctor.org/asciidoc/latest/macros/footnote/
#[derive(Clone, Debug)]
pub struct FootnoteRenderParams<'a> {
    /// The footnote's number, or `None` for an unresolved reference. Normally a
    /// consecutive integer, but the `footnote-number` counter honors any seed
    /// the document sets, so it is passed through as text.
    pub index: Option<&'a str>,

    /// The footnote's own ID, used only on a defining occurrence to produce the
    /// `id="_footnote_<id>"` attribute on the marker.
    pub id: Option<&'a str>,

    /// `true` when this occurrence references an existing footnote (or fails to
    /// resolve one); `false` for the defining occurrence.
    pub is_reference: bool,

    /// For an unresolved reference, the text to show inside the error marker
    /// (the unresolved ID). Ignored in the other cases.
    pub text: &'a str,
}

/// Implementation of [`InlineSubstitutionRenderer`] that renders substitutions
/// for common HTML-based applications.
#[derive(Debug)]
pub struct HtmlSubstitutionRenderer {}

impl InlineSubstitutionRenderer for HtmlSubstitutionRenderer {
    fn render_special_character(&self, type_: SpecialCharacter, dest: &mut String) {
        match type_ {
            SpecialCharacter::Lt => {
                dest.push_str("&lt;");
            }
            SpecialCharacter::Gt => {
                dest.push_str("&gt;");
            }
            SpecialCharacter::Ampersand => {
                dest.push_str("&amp;");
            }
        }
    }

    fn render_quoted_substitition(
        &self,
        type_: QuoteType,
        _scope: QuoteScope,
        attrlist: Option<Attrlist<'_>>,
        mut id: Option<String>,
        body: &str,
        dest: &mut String,
    ) {
        let mut roles: Vec<&str> = attrlist.as_ref().map(|a| a.roles()).unwrap_or_default();

        if let Some(block_style) = attrlist
            .as_ref()
            .and_then(|a| a.nth_attribute(1))
            .and_then(|attr1| attr1.block_style())
        {
            roles.insert(0, block_style);
        }

        if id.is_none() {
            id = attrlist
                .as_ref()
                .and_then(|a| a.nth_attribute(1))
                .and_then(|attr1| attr1.id())
                .map(|id| id.to_owned())
        }

        match type_ {
            QuoteType::Strong => {
                wrap_body_in_html_tag(attrlist.as_ref(), "strong", id, roles, body, dest);
            }

            QuoteType::DoubleQuote => {
                dest.push_str("&#8220;");
                dest.push_str(body);
                dest.push_str("&#8221;");
            }

            QuoteType::SingleQuote => {
                dest.push_str("&#8216;");
                dest.push_str(body);
                dest.push_str("&#8217;");
            }

            QuoteType::Monospaced => {
                wrap_body_in_html_tag(attrlist.as_ref(), "code", id, roles, body, dest);
            }

            QuoteType::Emphasis => {
                wrap_body_in_html_tag(attrlist.as_ref(), "em", id, roles, body, dest);
            }

            QuoteType::Mark => {
                if roles.is_empty() && id.is_none() {
                    wrap_body_in_html_tag(attrlist.as_ref(), "mark", id, roles, body, dest);
                } else {
                    wrap_body_in_html_tag(attrlist.as_ref(), "span", id, roles, body, dest);
                }
            }

            QuoteType::Superscript => {
                wrap_body_in_html_tag(attrlist.as_ref(), "sup", id, roles, body, dest);
            }

            QuoteType::Subscript => {
                wrap_body_in_html_tag(attrlist.as_ref(), "sub", id, roles, body, dest);
            }

            QuoteType::Unquoted => {
                if roles.is_empty() && id.is_none() {
                    dest.push_str(body);
                } else {
                    wrap_body_in_html_tag(attrlist.as_ref(), "span", id, roles, body, dest);
                }
            }

            QuoteType::AsciiMath => {
                dest.push_str(r"\$");
                dest.push_str(body);
                dest.push_str(r"\$");
            }

            QuoteType::LatexMath => {
                dest.push_str(r"\(");
                dest.push_str(body);
                dest.push_str(r"\)");
            }
        }
    }

    fn render_character_replacement(&self, type_: CharacterReplacementType, dest: &mut String) {
        match type_ {
            CharacterReplacementType::Copyright => {
                dest.push_str("&#169;");
            }

            CharacterReplacementType::Registered => {
                dest.push_str("&#174;");
            }

            CharacterReplacementType::Trademark => {
                dest.push_str("&#8482;");
            }

            CharacterReplacementType::EmDashSurroundedBySpaces => {
                dest.push_str("&#8201;&#8212;&#8201;");
            }

            CharacterReplacementType::EmDashWithoutSpace => {
                dest.push_str("&#8212;&#8203;");
            }

            CharacterReplacementType::Ellipsis => {
                dest.push_str("&#8230;&#8203;");
            }

            CharacterReplacementType::SingleLeftArrow => {
                dest.push_str("&#8592;");
            }

            CharacterReplacementType::DoubleLeftArrow => {
                dest.push_str("&#8656;");
            }

            CharacterReplacementType::SingleRightArrow => {
                dest.push_str("&#8594;");
            }

            CharacterReplacementType::DoubleRightArrow => {
                dest.push_str("&#8658;");
            }

            CharacterReplacementType::TypographicApostrophe => {
                dest.push_str("&#8217;");
            }

            CharacterReplacementType::CharacterReference(name) => {
                dest.push('&');
                dest.push_str(&name);
                dest.push(';');
            }
        }
    }

    fn render_line_break(&self, dest: &mut String) {
        dest.push_str("<br>");
    }

    fn render_image(&self, params: &ImageRenderParams, dest: &mut String) {
        let src = self.image_uri(params.target, params.parser, None);

        let mut attrs: Vec<String> = vec![
            format!(r#"src="{src}""#),
            format!(
                r#"alt="{alt}""#,
                alt = encode_attribute_value(params.alt.to_string())
            ),
        ];

        if let Some(width) = params.width {
            attrs.push(format!(r#"width="{width}""#));
        }

        if let Some(height) = params.height {
            attrs.push(format!(r#"height="{height}""#));
        }

        if let Some(title) = params.attrlist.named_attribute("title") {
            attrs.push(format!(
                r#"title="{title}""#,
                title = encode_attribute_value(title.value().to_owned())
            ));
        }

        let format = params
            .attrlist
            .named_attribute("format")
            .map(|format| format.value());

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/277):
        // Enforce non-safe mode. Add this contraint to following `if` clause:
        // `&& node.document.safe < SafeMode::SECURE`

        let img = if format == Some("svg") || params.target.contains(".svg") {
            // NOTE: In the SVG case we may have to ignore the attrs list.
            if params.attrlist.has_option("inline") {
                todo!(
                    "Port this: {}",
                    r#"img = (read_svg_contents node, target) || %(<span class="alt">#{node.alt}</span>)
                    NOTE: The attrs list calculated above may not be usable.
                    "#
                );
            } else if params.attrlist.has_option("interactive") {
                todo!(
                    "Port this: {}",
                    r##"
                        fallback = (node.attr? 'fallback') ? %(<img src="#{node.image_uri node.attr 'fallback'}" alt="#{encode_attribute_value node.alt}"#{attrs}#{@void_element_slash}>) : %(<span class="alt">#{node.alt}</span>)
                        img = %(<object type="image/svg+xml" data="#{src = node.image_uri target}"#{attrs}>#{fallback}</object>)
                        NOTE: The attrs list calculated above may not be usable.
                    "##
                );
            } else {
                format!(
                    r#"<img {attrs}{void_element_slash}>"#,
                    attrs = attrs.join(" "),
                    void_element_slash = "",
                )
            }
        } else {
            format!(
                r#"<img {attrs}{void_element_slash}>"#,
                attrs = attrs.join(" "),
                void_element_slash = "",
                // img = %(<img src="#{src = node.image_uri target}"
                // alt="#{encode_attribute_value node.alt}"#{attrs}#{@
                // void_element_slash}>)
            )
        };

        render_icon_or_image(params.attrlist, &img, &src, "image", dest);
    }

    fn image_uri(
        &self,
        target_image_path: &str,
        parser: &Parser,
        asset_dir_key: Option<&str>,
    ) -> String {
        let asset_dir_key = asset_dir_key.unwrap_or("imagesdir");

        if false {
            todo!(
                // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/277):
                "Port this when implementing safe modes: {}",
                r#"
				if (doc = @document).safe < SafeMode::SECURE && (doc.attr? 'data-uri')
				  if ((Helpers.uriish? target_image) && (target_image = Helpers.encode_spaces_in_uri target_image)) ||
					  (asset_dir_key && (images_base = doc.attr asset_dir_key) && (Helpers.uriish? images_base) &&
					  (target_image = normalize_web_path target_image, images_base, false))
					(doc.attr? 'allow-uri-read') ? (generate_data_uri_from_uri target_image, (doc.attr? 'cache-uri')) : target_image
				  else
					generate_data_uri target_image, asset_dir_key
				  end
				else
				  normalize_web_path target_image, (asset_dir_key ? (doc.attr asset_dir_key) : nil)
				end
            "#
            );
        } else {
            let asset_dir = parser
                .attribute_value(asset_dir_key)
                .as_maybe_str()
                .map(|s| s.to_string());

            normalize_web_path(target_image_path, parser, asset_dir.as_deref(), true)
        }
    }

    fn render_icon(&self, params: &IconRenderParams, dest: &mut String) {
        let src = self.icon_uri(params.target, params.attrlist, params.parser);

        let img = if params.parser.is_attribute_set("icons") {
            let icons = params.parser.attribute_value("icons");
            if let Some(icons) = icons.as_maybe_str()
                && icons == "font"
            {
                let mut i_class_attrs: Vec<String> = vec![
                    "fa".to_owned(),
                    format!("fa-{target}", target = params.target),
                ];

                if let Some(size) = params.attrlist.named_or_positional_attribute("size", 1) {
                    i_class_attrs.push(format!("fa-{size}", size = size.value()));
                }

                if let Some(flip) = params.attrlist.named_attribute("flip") {
                    i_class_attrs.push(format!("fa-flip-{flip}", flip = flip.value()));
                } else if let Some(rotate) = params.attrlist.named_attribute("rotate") {
                    i_class_attrs.push(format!("fa-rotate-{rotate}", rotate = rotate.value()));
                }

                format!(
                    r##"<i class="{i_class_attr_val}"{title_attr}></i>"##,
                    i_class_attr_val = i_class_attrs.join(" "),
                    title_attr = if let Some(title) = params.attrlist.named_attribute("title") {
                        format!(r#" title="{title}""#, title = title.value())
                    } else {
                        "".to_owned()
                    }
                )
            } else {
                let mut attrs: Vec<String> = vec![
                    format!(r#"src="{src}""#),
                    format!(
                        r#"alt="{alt}""#,
                        alt = encode_attribute_value(params.alt.to_string())
                    ),
                ];

                if let Some(width) = params.attrlist.named_attribute("width") {
                    attrs.push(format!(r#"width="{width}""#, width = width.value()));
                }

                if let Some(height) = params.attrlist.named_attribute("height") {
                    attrs.push(format!(r#"height="{height}""#, height = height.value()));
                }

                if let Some(title) = params.attrlist.named_attribute("title") {
                    attrs.push(format!(r#"title="{title}""#, title = title.value()));
                }

                format!(
                    "<img {attrs}{void_element_slash}>",
                    attrs = attrs.join(" "),
                    void_element_slash = "",
                )
            }
        } else {
            format!("[{alt}&#93;", alt = params.alt)
        };

        render_icon_or_image(params.attrlist, &img, &src, "icon", dest);
    }

    fn render_link(&self, params: &LinkRenderParams, dest: &mut String) {
        let id = params.attrlist.id();

        let mut roles = params.extra_roles.clone();
        let mut attrlist_roles = params.attrlist.roles().clone();
        roles.append(&mut attrlist_roles);

        let link = format!(
            r##"<a href="{target}"{id}{class}{link_constraint_attrs}>{link_text}</a>"##,
            target = params.target,
            id = if let Some(id) = id {
                format!(r#" id="{id}""#)
            } else {
                "".to_owned()
            },
            class = if roles.is_empty() {
                "".to_owned()
            } else {
                format!(r#" class="{roles}""#, roles = roles.join(" "))
            },
            // title = %( title="#{node.attr 'title'}") if node.attr? 'title'
            // Haven't seen this in the wild yet.
            link_constraint_attrs = link_constraint_attrs(params.attrlist, params.window),
            link_text = params.link_text,
        );

        dest.push_str(&link);
    }

    fn render_anchor(&self, id: &str, _reftext: Option<String>, dest: &mut String) {
        dest.push_str(&format!("<a id=\"{id}\"></a>"));
    }

    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
        match params.resolved {
            Some(resolved) => {
                let text = params
                    .provided_text
                    .map(str::to_string)
                    .or_else(|| resolved.text.clone())
                    .unwrap_or_else(|| format!("[{target}]", target = params.target));

                dest.push_str(&format!(
                    r#"<a href="{href}">{text}</a>"#,
                    href = resolved.href
                ));
            }

            None => {
                // Unresolved: link to the raw target and show bracketed text,
                // mirroring Asciidoctor's behavior for a missing reference.
                let text = params
                    .provided_text
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("[{target}]", target = params.target));

                dest.push_str(&format!(
                    r##"<a href="#{target}">{text}</a>"##,
                    target = params.target
                ));
            }
        }
    }

    fn render_callout(&self, params: &CalloutRenderParams, dest: &mut String) {
        let n = params.number;
        let parser = params.parser;

        if parser.attribute_value("icons").as_maybe_str() == Some("font") {
            dest.push_str(&format!(
                r#"<i class="conum" data-value="{n}"></i><b>({n})</b>"#
            ));
        } else if parser.is_attribute_set("icons") {
            let icontype = parser
                .attribute_value("icontype")
                .as_maybe_str()
                .unwrap_or("png")
                .to_owned();

            let icon = format!("callouts/{n}.{icontype}");
            let src = self.image_uri(&icon, parser, Some("iconsdir"));

            dest.push_str(&format!(r#"<img src="{src}" alt="{n}">"#));
        } else {
            match params.guard {
                CalloutGuard::Xml => {
                    dest.push_str(&format!(r#"&lt;!--<b class="conum">({n})</b>--&gt;"#));
                }

                CalloutGuard::LineComment(prefix) => {
                    dest.push_str(prefix);
                    dest.push_str(&format!(r#"<b class="conum">({n})</b>"#));
                }
            }
        }
    }

    fn render_index_term(&self, params: &IndexTermRenderParams, dest: &mut String) {
        // The HTML5 converter does not generate an index, so a concealed index
        // term produces no output and a flow index term renders only its
        // (already-substituted) visible term text.
        if let Some(term) = params.visible_term {
            dest.push_str(term);
        }
    }

    fn render_footnote(&self, params: &FootnoteRenderParams, dest: &mut String) {
        match params.index {
            Some(index) if params.is_reference => {
                // A reference to an already-defined footnote reuses its number
                // but gets no anchor of its own.
                dest.push_str(&format!(
                    r##"<sup class="footnoteref">[<a class="footnote" href="#_footnotedef_{index}" title="View footnote.">{index}</a>]</sup>"##
                ));
            }

            Some(index) => {
                // A defining occurrence. When the footnote carries an ID, the
                // marker is given a matching anchor so it can be linked to.
                let id_attr = params
                    .id
                    .map(|id| format!(r#" id="_footnote_{id}""#))
                    .unwrap_or_default();

                dest.push_str(&format!(
                    r##"<sup class="footnote"{id_attr}>[<a id="_footnoteref_{index}" class="footnote" href="#_footnotedef_{index}" title="View footnote.">{index}</a>]</sup>"##
                ));
            }

            None => {
                // An unresolved reference: the ID was never defined.
                dest.push_str(&format!(
                    r#"<sup class="footnoteref red" title="Unresolved footnote reference.">[{text}]</sup>"#,
                    text = params.text
                ));
            }
        }
    }
}

fn wrap_body_in_html_tag(
    _attrlist: Option<&Attrlist<'_>>,
    tag: &'static str,
    id: Option<String>,
    roles: Vec<&str>,
    body: &str,
    dest: &mut String,
) {
    dest.push('<');
    dest.push_str(tag);

    if let Some(id) = id.as_ref() {
        dest.push_str(" id=\"");
        dest.push_str(id);
        dest.push('"');
    }

    if !roles.is_empty() {
        let roles = roles.join(" ");
        dest.push_str(" class=\"");
        dest.push_str(&roles);
        dest.push('"');
    }

    dest.push('>');
    dest.push_str(body);
    dest.push_str("</");
    dest.push_str(tag);
    dest.push('>');
}

fn render_icon_or_image(
    attrlist: &Attrlist,
    img: &str,
    src: &str,
    type_: &'static str,
    dest: &mut String,
) {
    let mut img = img.to_string();

    if let Some(link) = attrlist.named_attribute("link") {
        let mut link = link.value();
        if link == "self" {
            link = src;
        }

        img = format!(
            r#"<a class="image" href="{link}"{link_constraint_attrs}>{img}</a>"#,
            link_constraint_attrs = link_constraint_attrs(attrlist, None)
        );
    }

    let mut roles: Vec<&str> = attrlist.roles();

    if let Some(float) = attrlist.named_attribute("float") {
        roles.insert(0, float.value());
    }

    roles.insert(0, type_);

    dest.push_str(r#"<span class=""#);
    dest.push_str(&roles.join(" "));
    dest.push_str(r#"">"#);
    dest.push_str(&img);
    dest.push_str("</span>");
}

fn encode_attribute_value(value: String) -> String {
    value.replace('"', "&quot;")
}

fn normalize_web_path(
    target: &str,
    parser: &Parser,
    start: Option<&str>,
    preserve_uri_target: bool,
) -> String {
    if preserve_uri_target && is_uri_ish(target) {
        encode_spaces_in_uri(target)
    } else {
        parser.path_resolver.web_path(target, start)
    }
}

fn is_uri_ish(path: &str) -> bool {
    path.contains(':') && URI_SNIFF.is_match(path)
}

fn encode_spaces_in_uri(s: &str) -> String {
    s.replace(' ', "%20")
}

/// Detects strings that resemble URIs.
///
/// ## Examples
///
/// * `http://domain`
/// * `https://domain`
/// * `file:///path`
/// * `data:info`
///
/// ## Counter-examples (do not match)
///
/// * `c:/sample.adoc`
/// * `c:\sample.adoc`
static URI_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
        \A                             # Anchor to start of string
        \p{Alphabetic}                 # First character must be a letter
        [\p{Alphabetic}\p{Nd}.+-]+     # Followed by one or more alphanum or . + -
        :                              # Literal colon
        /{0,2}                         # Zero to two slashes
    "#,
    )
    .unwrap()
});

fn link_constraint_attrs(attrlist: &Attrlist<'_>, window: Option<&'static str>) -> String {
    let rel = if attrlist.has_option("nofollow") {
        Some("nofollow")
    } else {
        None
    };

    if let Some(window) = attrlist
        .named_attribute("window")
        .map(|a| a.value())
        .or(window)
    {
        let rel_noopener = if window == "_blank" || attrlist.has_option("noopener") {
            if let Some(rel) = rel {
                format!(r#" rel="{rel}" noopener"#)
            } else {
                r#" rel="noopener""#.to_owned()
            }
        } else {
            "".to_string()
        };

        format!(r#" target="{window}"{rel_noopener}"#)
    } else if let Some(rel) = rel {
        format!(r#" rel="{rel}""#)
    } else {
        "".to_string()
    }
}
