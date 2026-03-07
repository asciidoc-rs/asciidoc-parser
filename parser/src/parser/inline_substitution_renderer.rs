use std::{fmt::Debug, sync::LazyLock};

use regex::Regex;

use crate::{Parser, attributes::Attrlist};

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

        let img = if params.parser.has_attribute("icons") {
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
