use std::{fmt::Debug, sync::LazyLock};

use regex::Regex;

use crate::{
    Parser,
    attributes::Attrlist,
    parser::{DerivedReference, ResolvedReference, SafeMode, XrefSignifier, XrefStyle},
};

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
    /// If the `data-uri` attribute is set on the document and the safe mode is
    /// below `SafeMode::Secure`, the image is embedded as a
    /// `data:<mime>;base64,…` URI by reading its bytes through the
    /// [`ImageFileHandler`](crate::parser::ImageFileHandler); otherwise (or
    /// when no handler is registered) a normalized relative path (i.e.,
    /// URL) is returned. A target that is itself a URI is never embedded.
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
    /// The target image path is derived from the icon name. If the name already
    /// carries a file extension, it is used verbatim; otherwise the value of
    /// the `icontype` attribute (defaulting to `png`) is appended. In both
    /// cases the path is resolved relative to the `iconsdir` attribute.
    /// This mirrors the icon macro's image mode, where `icontype` is only
    /// consulted when the icon type must be inferred (i.e. the target has
    /// no file extension).
    ///
    /// The target image path is then passed through the `image_uri()` method.
    /// If the `data-uri` attribute is set on the document, the image will be
    /// safely converted to a data URI.
    ///
    /// The return value of this method can be safely used in an image tag.
    fn icon_uri(&self, name: &str, _attrlist: &Attrlist, parser: &Parser) -> String {
        let icon = if has_extname(name) {
            name.to_owned()
        } else {
            let icontype = parser
                .attribute_value("icontype")
                .as_maybe_str()
                .unwrap_or("png")
                .to_owned();

            format!("{name}.{icontype}")
        };

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

    /// Renders a [button] UI macro (`btn:[label]`).
    ///
    /// `text` is the already-normalized button label. The renderer should write
    /// an appropriate rendering (e.g. `<b class="button">label</b>`) to `dest`.
    ///
    /// [button]: https://docs.asciidoctor.org/asciidoc/latest/macros/ui-macros/
    fn render_button(&self, text: &str, dest: &mut String);

    /// Renders a [keyboard] UI macro (`kbd:[keys]`).
    ///
    /// `keys` holds one entry per key in the shortcut. A single-element slice
    /// is a lone key; multiple entries form a key sequence. The renderer
    /// should write an appropriate rendering (e.g. a lone `<kbd>` element,
    /// or a `<span class="keyseq">` wrapping several `<kbd>` elements) to
    /// `dest`.
    ///
    /// [keyboard]: https://docs.asciidoctor.org/asciidoc/latest/macros/keyboard-macro/
    fn render_keyboard(&self, keys: &[String], dest: &mut String);

    /// Renders a [menu] UI macro (`menu:menu[submenu > … > item]`).
    ///
    /// The renderer should write an appropriate rendering to `dest`.
    ///
    /// [menu]: https://docs.asciidoctor.org/asciidoc/latest/macros/ui-macros/
    fn render_menu(&self, params: &MenuRenderParams, dest: &mut String);

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

    /// Target window selection from a `window` attribute on the `xref:` macro
    /// (e.g. `_blank`), or `None`. When `_blank`, the renderer also emits
    /// `rel="noopener"`, mirroring the link macro.
    pub window: Option<&'a str>,

    /// Roles supplied via a `role` attribute on the `xref:` macro. Empty when
    /// none were given.
    pub roles: &'a [String],

    /// The cross-reference text style in effect for this reference (from the
    /// `xrefstyle=` macro attribute or the document-wide `xrefstyle`). `None`
    /// when `xrefstyle` is unset, in which case the target's reference text is
    /// used verbatim.
    pub xrefstyle: Option<XrefStyle>,

    /// The destination the parser derived from the target itself, for a
    /// target that names a document; `None` for a reference to an element
    /// within the current document.
    ///
    /// This is what the reference renders as when
    /// [`resolved`](Self::resolved) is `None`: such a target is not
    /// unresolved, it simply resolves without the catalog's help.
    pub derived: Option<&'a DerivedReference>,

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

/// Provides parameters for rendering a [menu] UI macro.
///
/// [menu]: https://docs.asciidoctor.org/asciidoc/latest/macros/ui-macros/
#[derive(Clone, Debug)]
pub struct MenuRenderParams<'a> {
    /// The top-level menu name.
    pub menu: &'a str,

    /// Zero or more intermediate submenu names, in order from outermost to
    /// innermost.
    pub submenus: &'a [String],

    /// The final menu item, if any. `None` renders a bare menu reference (a
    /// `menu:File[]` with no items).
    pub menuitem: Option<&'a str>,

    /// Parser, used to read the `icons` document attribute when choosing how to
    /// render the caret between menu levels.
    pub parser: &'a Parser,
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

impl HtmlSubstitutionRenderer {
    /// Resolve an image target to a `src`/`data` reference, honoring a
    /// macro-level `imagesdir` attribute.
    ///
    /// A named `imagesdir` attribute _on the image macro itself_ overrides the
    /// document `imagesdir` for this one image (Asciidoctor 2.1+). When it is
    /// absent, resolution falls back to [`image_uri`], which uses the document
    /// `imagesdir`. As with the document attribute, an absolute-URL target
    /// ignores the base entirely.
    ///
    /// [`image_uri`]: InlineSubstitutionRenderer::image_uri
    fn image_src(&self, target: &str, attrlist: &Attrlist, parser: &Parser) -> String {
        match attrlist.named_attribute("imagesdir") {
            Some(imagesdir) => normalize_web_path(target, parser, Some(imagesdir.value()), true),
            None => self.image_uri(target, parser, None),
        }
    }
}

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
        let src = self.image_src(params.target, params.attrlist, params.parser);
        let alt_encoded = encode_attribute_value(params.alt.clone());

        // The dimension attributes (width, height, and title) are shared by the
        // plain `<img>`, the interactive `<object>`, and the `<object>`'s image
        // fallback. Each fragment carries its own leading space so the pieces
        // concatenate cleanly after `src`/`alt` (or the `data` attribute).
        let mut dimension_attrs = String::new();

        if let Some(width) = params.width {
            dimension_attrs.push_str(&format!(
                r#" width="{width}""#,
                width = encode_attribute_value(width.to_owned())
            ));
        }

        if let Some(height) = params.height {
            dimension_attrs.push_str(&format!(
                r#" height="{height}""#,
                height = encode_attribute_value(height.to_owned())
            ));
        }

        if let Some(title) = params.attrlist.named_attribute("title") {
            dimension_attrs.push_str(&format!(
                r#" title="{title}""#,
                title = encode_attribute_value(title.value().to_owned())
            ));
        }

        let format = params
            .attrlist
            .named_attribute("format")
            .map(|format| format.value());

        // The `inline` and `interactive` SVG options are security-sensitive
        // (they embed file contents or a live `<object>`), so they only take
        // effect below the `Secure` safe mode. In `Secure` mode an SVG image
        // renders as an ordinary `<img>`, matching Ruby Asciidoctor.
        let svg_active = (format == Some("svg") || params.target.contains(".svg"))
            && params.parser.safe_mode() < SafeMode::Secure;

        // An inline SVG is embedded verbatim and has no meaningful `src`, so a
        // `link=self` on it is left as the literal `self` rather than resolved
        // to a URI (see `render_icon_or_image`). Every other image form does
        // have a `src` (a data URI or web path) that `link=self` resolves to.
        let inline_svg = svg_active && params.attrlist.has_option("inline");

        let img = if inline_svg {
            // Embed the SVG contents directly. When the contents cannot be read
            // (no handler is registered, or it cannot find the file), fall back
            // to the alt text, mirroring Ruby Asciidoctor.
            read_svg_contents(&src, params.width, params.height, params.parser)
                .unwrap_or_else(|| format!(r#"<span class="alt">{alt}</span>"#, alt = params.alt))
        } else if svg_active && params.attrlist.has_option("interactive") {
            // Render an interactive SVG as an `<object>` element so its embedded
            // scripting and links remain live. A `fallback` image (or, failing
            // that, the alt text) is nested inside for user agents that can't
            // display the object.
            let fallback = if let Some(fallback) = params.attrlist.named_attribute("fallback") {
                let fallback_src = self.image_src(fallback.value(), params.attrlist, params.parser);
                format!(
                    r#"<img src="{fallback_src}" alt="{alt_encoded}"{dimension_attrs}>"#,
                    fallback_src = encode_attribute_value(fallback_src)
                )
            } else {
                format!(r#"<span class="alt">{alt}</span>"#, alt = params.alt)
            };

            format!(
                r#"<object type="image/svg+xml" data="{src}"{dimension_attrs}>{fallback}</object>"#,
                src = encode_attribute_value(src.clone())
            )
        } else {
            format!(
                r#"<img src="{src}" alt="{alt_encoded}"{dimension_attrs}>"#,
                src = encode_attribute_value(src.clone())
            )
        };

        let link_self_href = if inline_svg { None } else { Some(src.as_str()) };

        render_icon_or_image(params.attrlist, &img, "image", link_self_href, dest);
    }

    fn image_uri(
        &self,
        target_image_path: &str,
        parser: &Parser,
        asset_dir_key: Option<&str>,
    ) -> String {
        let asset_dir_key = asset_dir_key.unwrap_or("imagesdir");

        let asset_dir = parser
            .attribute_value(asset_dir_key)
            .as_maybe_str()
            .map(|s| s.to_string());

        let normalized = normalize_web_path(target_image_path, parser, asset_dir.as_deref(), true);

        // Asciidoctor embeds the image as a data URI when the `data-uri`
        // attribute is set and the safe mode is below `SafeMode::Secure`. A
        // target that is itself a URI is never embedded – there is no local
        // file to read – so it passes through as an ordinary web path
        // (Asciidoctor only fetches a remote target under `allow-uri-read`,
        // which this crate does not implement). Otherwise the image's bytes are
        // read through the `ImageFileHandler` and base64-encoded into a
        // `data:<mime>;base64,…` URI.
        //
        // This crate never performs file I/O itself, so an absent handler (or
        // one that cannot find the file) degrades silently to the web path,
        // mirroring how a missing `SvgFileHandler` degrades an inline SVG.
        if parser.safe_mode() < SafeMode::Secure
            && parser.is_attribute_set("data-uri")
            && !is_uri_ish(target_image_path)
            && let Some(handler) = parser.image_file_handler.as_ref()
            && let Some(bytes) = handler.resolve_image(&normalized, parser)
        {
            let mimetype = data_uri_mimetype(target_image_path);
            let encoded = crate::internal::base64::strict_encode(&bytes);

            return format!("data:{mimetype};base64,{encoded}");
        }

        normalized
    }

    fn render_icon(&self, params: &IconRenderParams, dest: &mut String) {
        let src = self.icon_uri(params.target, params.attrlist, params.parser);

        let img = if params.parser.is_attribute_set("icons") {
            let icons = params.parser.attribute_value("icons");
            if let Some(icons) = icons.as_maybe_str()
                && icons == "font"
            {
                // Every fragment interpolated into the `class`/`title`
                // attributes below is escaped for the `"` delimiter, mirroring
                // the `alt` handling in the image branch. These values are
                // already special-character-escaped (`< > &`) upstream, but a
                // stray `"` in an author-supplied `target`, `size`, `flip`,
                // `rotate`, or `title` would otherwise break out of its
                // attribute.
                let mut i_class_attrs: Vec<String> = vec![
                    "fa".to_owned(),
                    format!(
                        "fa-{target}",
                        target = encode_attribute_value(params.target.to_owned())
                    ),
                ];

                if let Some(size) = params.attrlist.named_or_positional_attribute("size", 1) {
                    i_class_attrs.push(format!(
                        "fa-{size}",
                        size = encode_attribute_value(size.value().to_owned())
                    ));
                }

                if let Some(flip) = params.attrlist.named_attribute("flip") {
                    i_class_attrs.push(format!(
                        "fa-flip-{flip}",
                        flip = encode_attribute_value(flip.value().to_owned())
                    ));
                } else if let Some(rotate) = params.attrlist.named_attribute("rotate") {
                    i_class_attrs.push(format!(
                        "fa-rotate-{rotate}",
                        rotate = encode_attribute_value(rotate.value().to_owned())
                    ));
                }

                format!(
                    r##"<i class="{i_class_attr_val}"{title_attr}></i>"##,
                    i_class_attr_val = i_class_attrs.join(" "),
                    title_attr = if let Some(title) = params.attrlist.named_attribute("title") {
                        format!(
                            r#" title="{title}""#,
                            title = encode_attribute_value(title.value().to_owned())
                        )
                    } else {
                        "".to_owned()
                    }
                )
            } else {
                let mut attrs: Vec<String> = vec![
                    format!(r#"src="{src}""#, src = encode_attribute_value(src.clone())),
                    format!(
                        r#"alt="{alt}""#,
                        alt = encode_attribute_value(params.alt.to_string())
                    ),
                ];

                if let Some(width) = params.attrlist.named_attribute("width") {
                    attrs.push(format!(
                        r#"width="{width}""#,
                        width = encode_attribute_value(width.value().to_owned())
                    ));
                }

                if let Some(height) = params.attrlist.named_attribute("height") {
                    attrs.push(format!(
                        r#"height="{height}""#,
                        height = encode_attribute_value(height.value().to_owned())
                    ));
                }

                if let Some(title) = params.attrlist.named_attribute("title") {
                    attrs.push(format!(
                        r#"title="{title}""#,
                        title = encode_attribute_value(title.value().to_owned())
                    ));
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

        // `src` is only a real image URI in the image-icon branch (icons enabled
        // and not font-based); the font (`<i>`) and text (`[alt]`) branches have
        // no `src`, so a `link=self` on them stays literal (see
        // `render_icon_or_image`).
        let link_self_href = if params.parser.is_attribute_set("icons")
            && params.parser.attribute_value("icons").as_maybe_str() != Some("font")
        {
            Some(src.as_str())
        } else {
            None
        };

        render_icon_or_image(params.attrlist, &img, "icon", link_self_href, dest);
    }

    fn render_link(&self, params: &LinkRenderParams, dest: &mut String) {
        let id = params.attrlist.id();

        let mut roles = params.extra_roles.clone();
        let mut attrlist_roles = params.attrlist.roles().clone();
        roles.append(&mut attrlist_roles);

        let link = format!(
            r##"<a href="{target}"{id}{class}{title}{link_constraint_attrs}>{link_text}</a>"##,
            // The target arrives here already special-character-escaped (`< > &`)
            // by the substitution pipeline, but that step leaves `"` intact. A
            // stray `"` in the target would otherwise close the `href` attribute
            // and let an author inject further attributes (e.g. an event
            // handler), so escape the quote delimiter here – mirroring the
            // image `alt`/`title` handling.
            target = encode_attribute_value(params.target.clone()),
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
            // Mirrors Asciidoctor's HTML5 converter: `title="#{node.attr 'title'}"`
            // is emitted (after the class) when the link carries a `title`
            // attribute.
            title = if let Some(title) = params.attrlist.named_attribute("title") {
                format!(
                    r#" title="{title}""#,
                    title = encode_attribute_value(title.value().to_owned())
                )
            } else {
                "".to_owned()
            },
            link_constraint_attrs = link_constraint_attrs(params.attrlist, params.window),
            link_text = params.link_text,
        );

        dest.push_str(&link);
    }

    fn render_anchor(&self, id: &str, _reftext: Option<String>, dest: &mut String) {
        dest.push_str(&format!("<a id=\"{id}\"></a>"));
    }

    fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
        let class = if params.roles.is_empty() {
            String::new()
        } else {
            // Roles are author-supplied, so each is escaped before it is joined
            // into the `class` attribute (a stray `"` would otherwise break out
            // of the attribute).
            let roles = params
                .roles
                .iter()
                .map(|role| encode_html_attribute(role))
                .collect::<Vec<_>>()
                .join(" ");
            format!(r#" class="{roles}""#)
        };

        let constraint_attrs = xref_constraint_attrs(params.window);

        // Each `href` below is escaped for the `"` delimiter before it is
        // interpolated into the attribute. The destinations are already
        // special-character-escaped (`< > &`) upstream, but a stray `"` in a
        // crafted or unresolved target would otherwise break out of the `href`
        // attribute (see the `class`/roles escaping above).
        match (params.resolved, params.derived) {
            (Some(resolved), _) => {
                // Explicit link text always wins; otherwise use the target's
                // reference text, optionally reformatted by the `xrefstyle`.
                // Empty explicit text (`<<id,>>`) is treated as absent, matching
                // Asciidoctor's fallback to the target's reference text.
                let text = match params.provided_text {
                    Some(provided) if !provided.is_empty() => provided.to_string(),
                    _ => {
                        // The target's reference text becomes this reference's
                        // link text. When that reftext is itself a title
                        // containing a cross-reference (or an inline link), it
                        // carries a nested `<a>…</a>`; an anchor cannot legally
                        // nest inside another, so the inner anchor tags are
                        // dropped (keeping their text), mirroring Asciidoctor's
                        // `DropAnchorRx`. The bracketed fallback (`[id]`) has no
                        // anchors, so stripping only applies to a resolved
                        // reftext.
                        let base = resolved
                            .text
                            .as_deref()
                            .map(drop_anchor_tags)
                            .unwrap_or_else(|| format!("[{target}]", target = params.target));
                        apply_xrefstyle(params.xrefstyle, resolved.signifier.as_ref(), base)
                    }
                };

                dest.push_str(&format!(
                    r#"<a href="{href}"{class}{constraint_attrs}>{text}</a>"#,
                    href = encode_attribute_value(resolved.href.clone())
                ));
            }

            // A target that named a document, which no resolver claimed: use
            // the destination derived from the target itself.
            (None, Some(derived)) => {
                let text = params
                    .provided_text
                    .map(str::to_string)
                    .unwrap_or_else(|| derived.text.clone());

                dest.push_str(&format!(
                    r#"<a href="{href}"{class}{constraint_attrs}>{text}</a>"#,
                    href = encode_attribute_value(derived.href.clone())
                ));
            }

            (None, None) => {
                // Unresolved: link to the raw target and show bracketed text,
                // mirroring Asciidoctor's behavior for a missing reference.
                let text = params
                    .provided_text
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("[{target}]", target = params.target));

                dest.push_str(&format!(
                    r##"<a href="#{target}"{class}{constraint_attrs}>{text}</a>"##,
                    target = encode_attribute_value(params.target.to_owned())
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

    fn render_button(&self, text: &str, dest: &mut String) {
        dest.push_str(&format!(r#"<b class="button">{text}</b>"#));
    }

    fn render_keyboard(&self, keys: &[String], dest: &mut String) {
        if let [key] = keys {
            dest.push_str(&format!("<kbd>{key}</kbd>"));
        } else {
            // The visual separator is always `+`, even when the source used a
            // comma delimiter (e.g. `kbd:[Ctrl,T]`). This matches Asciidoctor's
            // HTML5 output, where the delimiter only selects how keys are split,
            // not how the sequence is displayed.
            dest.push_str(&format!(
                r#"<span class="keyseq"><kbd>{keys}</kbd></span>"#,
                keys = keys.join("</kbd>+<kbd>")
            ));
        }
    }

    fn render_menu(&self, params: &MenuRenderParams, dest: &mut String) {
        let caret = if params.parser.attribute_value("icons").as_maybe_str() == Some("font") {
            r#"&#160;<i class="fa fa-angle-right caret"></i> "#
        } else {
            r#"&#160;<b class="caret">&#8250;</b> "#
        };

        let menu = params.menu;

        if params.submenus.is_empty() {
            if let Some(menuitem) = params.menuitem {
                dest.push_str(&format!(
                    r#"<span class="menuseq"><b class="menu">{menu}</b>{caret}<b class="menuitem">{menuitem}</b></span>"#
                ));
            } else {
                dest.push_str(&format!(r#"<b class="menuref">{menu}</b>"#));
            }
        } else {
            let submenu_joiner = format!(r#"</b>{caret}<b class="submenu">"#);
            dest.push_str(&format!(
                r#"<span class="menuseq"><b class="menu">{menu}</b>{caret}<b class="submenu">{submenus}</b>{caret}<b class="menuitem">{menuitem}</b></span>"#,
                submenus = params.submenus.join(&submenu_joiner),
                menuitem = params.menuitem.unwrap_or_default(),
            ));
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
    type_: &'static str,
    link_self_href: Option<&str>,
    dest: &mut String,
) {
    let mut img = img.to_string();

    // The `link` attribute value is used verbatim as the `href`, except that a
    // `link=self` resolves to the image's own `src` (its data URI or web path)
    // when one is available. An inline SVG (and a font/text icon) has no `src`
    // to resolve to, so `link_self_href` is `None` there and the literal `self`
    // is kept. (Ruby Asciidoctor, where `src` is undefined in those branches,
    // instead drops the anchor entirely; this crate keeps it with the literal
    // `self` target.)
    if let Some(link) = attrlist.named_attribute("link") {
        let is_self = link.value() == "self";

        let href = if is_self {
            link_self_href.unwrap_or("self")
        } else {
            link.value()
        };

        // An explicit `link=` destination whose scheme could execute script is
        // not turned into a live link; the image is rendered without the
        // wrapping anchor. Escaping the `"` delimiter alone would still leave a
        // live `javascript:` URI, so the destination is rejected outright – the
        // same policy the explicit `link:` macro applies, and the macro layer
        // records the accompanying warning. `link=self` resolves to the image's
        // own `src`, which may legitimately be a `data:image/*` URI, so it is
        // checked with the more permissive [`has_dangerous_self_href`] (a
        // `javascript:` or non-image `data:` target still resolves to a live
        // script URI here and is rejected).
        let rejected = if is_self {
            has_dangerous_self_href(href)
        } else {
            has_dangerous_scheme(link.value())
        };

        if !rejected {
            img = format!(
                r#"<a class="image" href="{href}"{link_constraint_attrs}>{img}</a>"#,
                // Both sources of `href` – the image's own `src` (a resolved web
                // path that can carry a stray `"`) and an author-supplied
                // `link=` value – are escaped for the `"` delimiter so neither
                // can break out of the attribute.
                href = encode_attribute_value(href.to_owned()),
                link_constraint_attrs = link_constraint_attrs(attrlist, None)
            );
        }
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

/// Reports whether `target` begins with a URI scheme that can execute script
/// when placed in an `href` – `javascript:`, `data:`, or `vbscript:`.
///
/// Leading control and space characters are ignored first, because a browser
/// strips them before it parses the scheme (so `"\u{1}javascript:…"` is still
/// live). The comparison is ASCII-case-insensitive.
///
/// Escaping the quote delimiter (see [`encode_attribute_value`]) stops an
/// author from breaking out of an attribute, but not from placing a live
/// script URI in an `href`; that requires rejecting the scheme outright. This
/// guards the explicit `link:` macro and the `image:`/`icon:` `link=`
/// attribute. The auto-linker already restricts bare URLs to a safe scheme set
/// (`https?`/`file`/`ftp`/`irc`).
pub(crate) fn has_dangerous_scheme(target: &str) -> bool {
    let target = target.trim_start_matches(|c: char| c <= ' ');

    const DANGEROUS_SCHEMES: [&str; 3] = ["javascript:", "data:", "vbscript:"];

    DANGEROUS_SCHEMES.iter().any(|scheme| {
        target
            .get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

/// Reports whether `href` – resolved from a `link=self` image/icon target –
/// would place a script-capable URI in the wrapping anchor's `href`.
///
/// `link=self` names the image's own `src`, so it cannot simply be run through
/// [`has_dangerous_scheme`]: an embedded image (`data-uri`) legitimately
/// resolves to a `data:image/*` URI, and there is an Asciidoctor-parity test
/// for exactly that. That one form is therefore exempt. Every other dangerous
/// scheme – `javascript:`, `vbscript:`, or a non-image `data:` such as
/// `data:text/html,…` – is never a valid image source, so promoting it into an
/// `href` is rejected: only the anchor is dropped (the harmless `<img src>` is
/// left intact), mirroring the `link=` policy above.
pub(crate) fn has_dangerous_self_href(href: &str) -> bool {
    has_dangerous_scheme(href) && !is_image_data_uri(href)
}

/// Reports whether `href` is a `data:image/*` URI (the leading control/space
/// characters are ignored and the comparison is ASCII-case-insensitive, as in
/// [`has_dangerous_scheme`]). This is the one `data:` form that is a legitimate
/// image source, so it is exempt from the `link=self` rejection above.
fn is_image_data_uri(href: &str) -> bool {
    let href = href.trim_start_matches(|c: char| c <= ' ');

    const IMAGE_DATA_PREFIX: &str = "data:image/";

    href.get(..IMAGE_DATA_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(IMAGE_DATA_PREFIX))
}

/// Escapes a value for safe interpolation into an HTML attribute.
///
/// Unlike [`encode_attribute_value`] (which only guards the quote delimiter to
/// mirror Asciidoctor's image-alt handling), this escapes the full set of
/// characters that could break out of, or corrupt, an attribute value. It is
/// used for author-supplied `xref` `window`/`role` values, which – unlike the
/// hard-coded `window` strings the link macro passes – can contain arbitrary
/// text.
fn encode_html_attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
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

/// Returns the file extension (including the leading `.`) of the final path
/// segment of `path`, or `None` when that segment carries no extension (its `.`
/// is the first or last character of the segment, or there is no `.`). Mirrors
/// Asciidoctor's `Helpers.extname`.
fn extname(path: &str) -> Option<&str> {
    let segment = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match segment.rfind('.') {
        Some(i) if i > 0 && i < segment.len() - 1 => Some(&segment[i..]),
        _ => None,
    }
}

/// Reports whether the final path segment of `path` carries a file extension.
/// Mirrors Asciidoctor's `Helpers.extname?`, used by the icon macro to decide
/// whether the `icontype` attribute should be appended.
fn has_extname(path: &str) -> bool {
    extname(path).is_some()
}

/// Determines the MIME type for a `data:` URI from the target image's file
/// extension, mirroring Asciidoctor's `generate_data_uri`: `.svg` maps to
/// `image/svg+xml`, any other extension maps to `image/<ext>`, and a target
/// with no extension maps to `application/octet-stream`.
///
/// The `image/<ext>` mapping is verbatim, matching Asciidoctor: `.jpg` yields
/// `image/jpg` (not the IANA-registered `image/jpeg`), while `.jpeg` yields
/// `image/jpeg`. This parity with Asciidoctor is deliberate.
fn data_uri_mimetype(target: &str) -> String {
    match extname(target) {
        Some(".svg") => "image/svg+xml".to_string(),

        // `extname` always includes the leading `.`, which is dropped here.
        Some(ext) => format!("image/{ext}", ext = ext.strip_prefix('.').unwrap_or(ext)),
        None => "application/octet-stream".to_string(),
    }
}

fn encode_spaces_in_uri(s: &str) -> String {
    s.replace(' ', "%20")
}

/// Matches the opening `<svg …>` tag at the start of an SVG document.
///
/// Like Ruby Asciidoctor's equivalent (`/\A<svg[^>]*>/`), the `[^>]*` stops at
/// the first `>`, so a `>` appearing unencoded inside an attribute value would
/// truncate the match. That cannot happen in well-formed XML (where `>` must be
/// written as `&gt;`), so this only affects malformed input, and then only by
/// leaving the opening tag's dimensions unrewritten.
static SVG_START_TAG_RX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"\A<svg[^>]*>").unwrap()
});

/// Matches a `width`, `height`, or `style` attribute (with its leading
/// whitespace) so they can be stripped from an SVG's opening tag.
static SVG_SNIFF_WIDTH_HEIGHT_RX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?s)\s+(?:width|height|style)=(?:"[^"]*"|'[^']*')"#).unwrap()
});

/// Reads and prepares the raw contents of an SVG file for inline embedding
/// (`image:target.svg[opts=inline]`).
///
/// The SVG contents are supplied by the parser's [`SvgFileHandler`]; when no
/// handler is registered (or it can't find the file) this returns `None` and
/// the caller falls back to rendering the alt text.
///
/// Before returning, the contents are prepared to match Ruby Asciidoctor:
///
/// * any XML preamble or doctype preceding the `<svg>` tag is removed, and
/// * if an explicit `width` and/or `height` was supplied on the macro, the
///   opening `<svg>` tag's own `width`, `height`, and `style` attributes are
///   dropped and the requested dimensions are appended in their place.
///
/// [`SvgFileHandler`]: crate::parser::SvgFileHandler
fn read_svg_contents(
    src: &str,
    width: Option<&str>,
    height: Option<&str>,
    parser: &Parser,
) -> Option<String> {
    let handler = parser.svg_file_handler.as_ref()?;
    let mut svg = handler.resolve_svg(src, parser)?;

    // Strip anything that precedes the opening `<svg>` tag (e.g. `<?xml … ?>`).
    if svg.starts_with('<')
        && let Some(start) = svg.find("<svg")
        && start > 0
    {
        svg = svg[start..].to_string();
    }

    // Rewrite the opening tag's dimensions only when at least one was supplied.
    if (width.is_some() || height.is_some())
        && let Some(start_tag) = SVG_START_TAG_RX.find(&svg).map(|m| m.as_str().to_string())
    {
        let rest = svg[start_tag.len()..].to_string();

        // Attributes between `<svg` and the closing `>`, with any existing
        // width/height/style removed.
        let inner = &start_tag[4..start_tag.len() - 1];
        let mut new_tag = format!("<svg{}", SVG_SNIFF_WIDTH_HEIGHT_RX.replace_all(inner, ""));

        if let Some(width) = width {
            new_tag.push_str(&format!(r#" width="{width}""#));
        }

        if let Some(height) = height {
            new_tag.push_str(&format!(r#" height="{height}""#));
        }

        new_tag.push('>');
        svg = format!("{new_tag}{rest}");
    }

    Some(svg)
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

/// Removes the anchor (`<a …>` / `</a>`) tags from `text`, keeping everything
/// between them.
///
/// Used when a cross-reference's link text is drawn from its target's reference
/// text and that reftext itself contains an anchor – an inline link, or a
/// cross-reference embedded in the target's title. HTML forbids nesting an
/// `<a>` inside another, so the inner anchor tags are stripped, leaving their
/// text in place. Mirrors Asciidoctor's `DropAnchorRx = /<(?:a\b[^>]*|\/a)>/`.
fn drop_anchor_tags(text: &str) -> String {
    // The common case – a reftext with no anchor at all – allocates a plain
    // copy and does no scanning.
    if !text.contains("<a") {
        return text.to_string();
    }

    #[allow(clippy::unwrap_used)]
    static DROP_ANCHOR_RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<(?:a\b[^>]*|/a)>").unwrap());

    DROP_ANCHOR_RX.replace_all(text, "").into_owned()
}

/// Builds the display text for a resolved cross-reference under the selected
/// [`XrefStyle`].
///
/// `base` is the target's reference text (its title, when the target has no
/// explicit reftext). Styling applies only when a style is selected *and* the
/// target carries an [`XrefSignifier`] (a numbered section or captioned block);
/// otherwise `base` is returned unchanged. The HTML conventions live here in
/// the HTML renderer: a title is wrapped in typographic quotes, except a
/// chapter or appendix title, which is emphasized with `<em>` (in every style).
fn apply_xrefstyle(
    style: Option<XrefStyle>,
    signifier: Option<&XrefSignifier>,
    base: String,
) -> String {
    let (Some(style), Some(signifier)) = (style, signifier) else {
        return base;
    };

    match style {
        XrefStyle::Full if signifier.emphasize => {
            format!("{label}, <em>{base}</em>", label = signifier.label)
        }
        XrefStyle::Full => {
            format!("{label}, &#8220;{base}&#8221;", label = signifier.label)
        }
        XrefStyle::Short => signifier.label.clone(),
        XrefStyle::Basic if signifier.emphasize => format!("<em>{base}</em>"),
        XrefStyle::Basic => base,
    }
}

/// Builds the `target`/`rel` attributes for a cross-reference whose `xref:`
/// macro carried a `window` attribute. Mirrors the link macro: a `_blank`
/// window automatically adds `rel="noopener"`.
fn xref_constraint_attrs(window: Option<&str>) -> String {
    let Some(window) = window else {
        return String::new();
    };

    let rel_noopener = if window == "_blank" {
        r#" rel="noopener""#
    } else {
        ""
    };

    // The `window` value is author-supplied, so it is escaped before being
    // interpolated into the `target` attribute. The `_blank` comparison above
    // runs on the raw value, which is correct for the well-formed inputs that
    // trigger `rel="noopener"`.
    format!(
        r#" target="{window}"{rel_noopener}"#,
        window = encode_html_attribute(window)
    )
}

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
                format!(r#" rel="{rel} noopener""#)
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

#[cfg(test)]
mod tests {
    use super::{data_uri_mimetype, drop_anchor_tags, encode_html_attribute, extname, has_extname};

    #[test]
    fn extname_extracts_final_segment_extension() {
        // A normal extension on the final path segment.
        assert_eq!(extname("fixtures/dot.gif"), Some(".gif"));
        assert_eq!(extname("circle.svg"), Some(".svg"));

        // A dot in an earlier segment does not count; only the final segment's
        // extension does.
        assert_eq!(extname("a.b/c"), None);

        // A leading or trailing dot in the segment is not an extension.
        assert_eq!(extname(".hidden"), None);
        assert_eq!(extname("trailing."), None);

        // No dot at all.
        assert_eq!(extname("plain"), None);

        // `has_extname` is the boolean form.
        assert!(has_extname("a/b.png"));
        assert!(!has_extname("a.b/c"));
    }

    #[test]
    fn data_uri_mimetype_maps_extension() {
        // `.svg` is special-cased; every other extension maps to `image/<ext>`.
        assert_eq!(data_uri_mimetype("circle.svg"), "image/svg+xml");
        assert_eq!(data_uri_mimetype("fixtures/dot.gif"), "image/gif");
        assert_eq!(data_uri_mimetype("photo.png"), "image/png");

        // The extension is used verbatim (matching Asciidoctor), so `.jpg`
        // yields `image/jpg` rather than the registered `image/jpeg`, while
        // `.jpeg` yields `image/jpeg`.
        assert_eq!(data_uri_mimetype("photo.jpg"), "image/jpg");
        assert_eq!(data_uri_mimetype("photo.jpeg"), "image/jpeg");

        // A target with no extension falls back to a generic binary type.
        assert_eq!(data_uri_mimetype("noext"), "application/octet-stream");
    }

    #[test]
    fn encode_html_attribute_escapes_special_characters() {
        // Each of the four characters that could break out of or corrupt an
        // HTML attribute value is replaced with its entity; ordinary characters
        // pass through untouched.
        assert_eq!(
            encode_html_attribute(r#"a&b"c<d>e"#),
            "a&amp;b&quot;c&lt;d&gt;e"
        );
        assert_eq!(encode_html_attribute("plain"), "plain");
    }

    #[test]
    fn drop_anchor_tags_strips_anchor_markup_keeping_text() {
        // Anchor-free text is returned unchanged.
        assert_eq!(drop_anchor_tags("plain text"), "plain text");

        // A single anchor's tags are removed, keeping the link text.
        assert_eq!(
            drop_anchor_tags(r#"Consult <a href="https://google.com">Google</a>"#),
            "Consult Google"
        );

        // A bracketed cross-reference fallback embedded in a reftext.
        assert_eq!(drop_anchor_tags(r##"B <a href="#a">[a]</a>"##), "B [a]");

        // Multiple anchors are all stripped.
        assert_eq!(
            drop_anchor_tags(r##"<a href="#x">X</a> and <a href="#y">Y</a>"##),
            "X and Y"
        );

        // A `<article>` tag is not an anchor and must be left intact (the `\b`
        // word boundary in the pattern keeps `<a` from matching `<article>`).
        assert_eq!(
            drop_anchor_tags("<article>text</article>"),
            "<article>text</article>"
        );
    }
}
