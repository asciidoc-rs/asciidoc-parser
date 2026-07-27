//! Regression tests for HTML-attribute-breakout and script-URI injection when
//! rendering untrusted AsciiDoc to HTML.
//!
//! These behaviors are a deliberate divergence from Ruby Asciidoctor (which
//! does neither the quote-escaping nor the scheme rejection exercised here), so
//! they live outside the `asciidoctor_rb` faithfulness suite.
//!
//! Two properties are covered:
//!
//!   * every `href`/`target`, the icon `class`/`title`/`width`/`height`
//!     attributes, and the quoted-text `class` (role) attribute escape the `"`
//!     delimiter, so a stray quote cannot close the attribute and inject
//!     further markup; and
//!   * the explicit `link:` macro rejects targets whose scheme could execute
//!     script (`javascript:`, `data:`, `vbscript:`), rendering them as inert
//!     source text rather than a live link.
//!
//! The values reaching the renderer are already special-character-escaped
//! (`< > &`) by the substitution pipeline, so the tests also confirm that the
//! added escaping does not double-escape those characters.

use crate::{
    Parser,
    blocks::{Block, FindBlocks, SimpleBlock},
    content::{Content, SubstitutionStep},
    parser::ModificationContext,
    warnings::WarningType,
};

/// Renders `src` as a document and returns the first paragraph's rendered HTML.
fn render_paragraph(src: &str) -> String {
    let mut parser = Parser::default();
    let doc = parser.parse(src);

    fn walk<'a>(mut blocks: impl Iterator<Item = &'a Block<'a>>) -> Option<&'a SimpleBlock<'a>> {
        blocks.find_map(|block| {
            if let Block::Simple(simple) = block {
                Some(simple)
            } else {
                walk(block.child_blocks())
            }
        })
    }

    walk(doc.child_blocks())
        .expect("expected at least one simple block")
        .content()
        .rendered()
        .to_string()
}

/// Renders `src` through the special-characters and macros substitution steps
/// (the two steps relevant to icon rendering) with the given `icons` attribute
/// value, mirroring the real pipeline's pre-escaping of `< > &`.
fn render_icon(src: &str, icons: &str) -> String {
    let mut content = Content::from(crate::Span::new(src));

    let parser =
        Parser::default().with_intrinsic_attribute("icons", icons, ModificationContext::ApiOnly);

    SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
    SubstitutionStep::Macros.apply(&mut content, &parser, None);

    content.rendered().to_string()
}

/// Renders `src` through the special-characters and macros substitution steps
/// with a default parser, mirroring the real pipeline's pre-escaping of
/// `< > &`. Used for the `image:` macro, whose `src`/`link` handling needs no
/// document attribute.
fn render_macros(src: &str) -> String {
    let mut content = Content::from(crate::Span::new(src));
    let parser = Parser::default();

    SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
    SubstitutionStep::Macros.apply(&mut content, &parser, None);

    content.rendered().to_string()
}

// ---- Link target escaping --------------------------------------------------

#[test]
fn link_target_escapes_quote_delimiter() {
    // A stray `"` in the target must not close the `href` attribute and let an
    // event handler be injected after it.
    assert_eq!(
        render_paragraph(r#"link:x"onmouseover=alert(1)//[click]"#),
        r#"<a href="x&quot;onmouseover=alert(1)//">click</a>"#
    );
}

#[test]
fn link_target_does_not_double_escape_special_characters() {
    // `< > &` arrive already escaped from the special-characters step; the
    // added quote-escaping must leave them singly escaped.
    assert_eq!(
        render_paragraph("link:/a<b>&c[txt]"),
        r#"<a href="/a&lt;b&gt;&amp;c">txt</a>"#
    );
}

#[test]
fn link_target_preserves_single_escaped_ampersand_in_query() {
    assert_eq!(
        render_paragraph("link:/search?a=1&b=2[txt]"),
        r#"<a href="/search?a=1&amp;b=2">txt</a>"#
    );
}

// ---- Cross-reference target escaping ---------------------------------------

#[test]
fn xref_unresolved_target_escapes_quote_delimiter() {
    assert_eq!(
        render_paragraph(r#"xref:foo"onmouseover=alert(1)//[txt]"#),
        r##"<a href="#foo&quot;onmouseover=alert(1)//">txt</a>"##
    );
}

#[test]
fn xref_unresolved_target_does_not_double_escape_special_characters() {
    assert_eq!(
        render_paragraph("xref:foo<bar>[txt]"),
        r##"<a href="#foo&lt;bar&gt;">txt</a>"##
    );
}

// ---- Quoted-text role (class) escaping -------------------------------------

#[test]
fn quoted_positional_role_class_escapes_quote_delimiter() {
    // A single-quoted positional role is applied verbatim (quotes included) to
    // the `class` attribute, so a `"` inside it must not close the attribute
    // and inject an event handler.
    assert_eq!(
        render_paragraph(r#"['x" onmouseover="y']*bold*"#),
        r#"<strong class="'x&quot; onmouseover=&quot;y'">bold</strong>"#
    );
}

#[test]
fn passthrough_quoted_positional_role_class_escapes_quote_delimiter() {
    // Same guarantee on the inline-passthrough restore path.
    assert_eq!(
        render_paragraph(r#"['x" onmouseover="y']++text++"#),
        r#"<span class="'x&quot; onmouseover=&quot;y'">text</span>"#
    );
}

#[test]
fn named_role_class_escapes_quote_delimiter() {
    // A single-quoted `role=` value can also carry a literal `"`.
    assert_eq!(
        render_paragraph(r#"[role='a"b']*bold*"#),
        r#"<strong class="a&quot;b">bold</strong>"#
    );
}

#[test]
fn quoted_positional_role_class_does_not_double_escape_special_characters() {
    // `< > &` arrive already escaped for the constrained path; the added
    // quote-escaping must leave them singly escaped.
    assert_eq!(
        render_paragraph("['a<b&c']*bold*"),
        r#"<strong class="'a&lt;b&amp;c'">bold</strong>"#
    );
}

// ---- Icon attribute escaping -----------------------------------------------

#[test]
fn icon_font_class_and_title_escape_quote_delimiter() {
    // `size` and `title` (escaped `"` inside quoted values) feed the
    // `class`/`title` attributes of the font `<i>`; each must escape the quote.
    assert_eq!(
        render_icon(r#"icon:home[size="2x\"y",title="a\"b"]"#, "font"),
        r#"<span class="icon"><i class="fa fa-home fa-2x&quot;y" title="a&quot;b"></i></span>"#
    );
}

#[test]
fn icon_image_dimensions_and_title_escape_quote_delimiter() {
    // width/height/title (escaped `"` inside quoted values) feed the `<img>`
    // attributes; each must escape the quote.
    assert_eq!(
        render_icon(r#"icon:home[width="1\"x",height="2\"y",title="a\"b"]"#, ""),
        r#"<span class="icon"><img src="./images/icons/home.png" alt="home" width="1&quot;x" height="2&quot;y" title="a&quot;b"></span>"#
    );
}

#[test]
fn icon_image_src_escapes_quote_delimiter() {
    // A `"` in the icon target reaches the resolved `src`; it must be escaped so
    // it cannot break out of the `src` attribute.
    assert_eq!(
        render_icon(r#"icon:a"x[]"#, ""),
        r#"<span class="icon"><img src="./images/icons/a&quot;x.png" alt="a&quot;x"></span>"#
    );
}

// ---- Image src / dimensions / link escaping --------------------------------

#[test]
fn image_src_escapes_quote_delimiter() {
    // A `"` in the image target reaches `src`; it must be escaped like the icon
    // `src` above (the `alt` beside it was already escaped).
    assert_eq!(
        render_macros(r#"image:a"x.png[alt]"#),
        r#"<span class="image"><img src="a&quot;x.png" alt="alt"></span>"#
    );
}

#[test]
fn image_width_escapes_quote_delimiter() {
    assert_eq!(
        render_macros(r#"image:x.png[alt,width="1\"y"]"#),
        r#"<span class="image"><img src="x.png" alt="alt" width="1&quot;y"></span>"#
    );
}

#[test]
fn image_link_href_escapes_quote_delimiter() {
    // An author-supplied `link=` value becomes the wrapping anchor's `href`.
    assert_eq!(
        render_macros(r#"image:x.png[alt,link="a\"b"]"#),
        r#"<span class="image"><a class="image" href="a&quot;b"><img src="x.png" alt="alt"></a></span>"#
    );
}

#[test]
fn image_link_self_escapes_src_and_href_without_double_escaping() {
    // `link=self` resolves the anchor `href` to the image's own `src`. Both the
    // `href` and the `src` must be escaped exactly once (a `&quot;`, never a
    // double-escaped `&amp;quot;`).
    assert_eq!(
        render_macros(r#"image:a"x.png[alt,link=self]"#),
        r#"<span class="image"><a class="image" href="a&quot;x.png"><img src="a&quot;x.png" alt="alt"></a></span>"#
    );
}

#[test]
fn image_link_with_dangerous_scheme_drops_the_wrapping_link() {
    // Escaping the `"` cannot neutralize a script URI, so a dangerous `link=`
    // destination is rejected: the image is rendered without the wrapping
    // anchor rather than with a live `javascript:` href.
    assert_eq!(
        render_macros("image:safe.png[alt,link=javascript:alert(1)]"),
        r#"<span class="image"><img src="safe.png" alt="alt"></span>"#
    );
}

#[test]
fn image_link_dangerous_scheme_records_a_warning() {
    let mut parser = Parser::default();
    let doc = parser.parse("image:safe.png[alt,link=javascript:alert(1)]");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
    );
}

#[test]
fn image_link_dangerous_scheme_rejection_is_case_insensitive() {
    assert_eq!(
        render_macros("image:safe.png[alt,link=JavaScript:alert(1)]"),
        r#"<span class="image"><img src="safe.png" alt="alt"></span>"#
    );
}

#[test]
fn icon_link_with_dangerous_scheme_drops_the_wrapping_link() {
    // The same rejection applies to the `icon:` macro's `link=` attribute.
    assert_eq!(
        render_icon("icon:home[link=javascript:alert(1)]", ""),
        r#"<span class="icon"><img src="./images/icons/home.png" alt="home"></span>"#
    );
}

#[test]
fn icon_link_self_with_dangerous_target_drops_the_wrapping_link() {
    // A dangerous `icon:` target that `link=self` would promote into the anchor
    // `href` is rejected too; the icon `<img>` is left intact. (The resolved
    // `src` gains the default `.png` icon extension.)
    assert_eq!(
        render_icon("icon:javascript:alert(1)[link=self]", ""),
        r#"<span class="icon"><img src="javascript:alert(1).png" alt="javascript:alert(1)"></span>"#
    );
}

#[test]
fn font_icon_link_self_with_dangerous_target_keeps_literal_self_without_warning() {
    // A font icon has no `src`, so `link=self` resolves to the literal `self`
    // (a harmless relative URL), not to the dangerous target. Nothing is
    // rejected, so no `UnsafeLinkSchemeRejected` warning is recorded.
    assert_eq!(
        render_icon("icon:javascript:alert(1)[link=self]", "font"),
        r#"<span class="icon"><a class="image" href="self"><i class="fa fa-javascript:alert(1)"></i></a></span>"#
    );

    let mut parser =
        Parser::default().with_intrinsic_attribute("icons", "font", ModificationContext::ApiOnly);
    let doc = parser.parse("icon:javascript:alert(1)[link=self]");

    assert_eq!(doc.warnings().count(), 0);
}

#[test]
fn image_link_self_is_not_rejected() {
    // `link=self` names the image's own (safe) `src`, so it stays a live link.
    assert_eq!(
        render_macros("image:safe.png[alt,link=self]"),
        r#"<span class="image"><a class="image" href="safe.png"><img src="safe.png" alt="alt"></a></span>"#
    );
}

#[test]
fn image_link_self_with_dangerous_target_drops_the_wrapping_link() {
    // A `javascript:` target is harmless in `<img src>` (a `src` does not
    // execute script), but `link=self` would otherwise promote it into a live
    // `href`. The anchor is dropped while the `<img>` is left intact.
    assert_eq!(
        render_macros("image:javascript:alert(1)[alt,link=self]"),
        r#"<span class="image"><img src="javascript:alert(1)" alt="alt"></span>"#
    );
}

#[test]
fn image_link_self_with_dangerous_target_records_a_warning() {
    let mut parser = Parser::default();
    let doc = parser.parse("image:javascript:alert(1)[alt,link=self]");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
    );
}

#[test]
fn image_link_self_with_data_text_html_target_drops_the_wrapping_link() {
    // A `data:` target is dual-use: `data:image/*` is a legitimate image
    // source, but `data:text/html,…` is not, and would be a live script URI in
    // the self-href. Only the anchor is dropped. (The `<` `>` reaching the
    // `src` are escaped by the special-characters step.)
    assert_eq!(
        render_macros("image:data:text/html,<script>alert(1)</script>[alt,link=self]"),
        r#"<span class="image"><img src="data:text/html,&lt;script&gt;alert(1)&lt;/script&gt;" alt="alt"></span>"#
    );
}

#[test]
fn image_link_self_dangerous_target_rejection_is_case_insensitive() {
    assert_eq!(
        render_macros("image:JavaScript:alert(1)[alt,link=self]"),
        r#"<span class="image"><img src="JavaScript:alert(1)" alt="alt"></span>"#
    );
}

#[test]
fn image_link_self_with_inline_raster_data_image_target_stays_a_live_link() {
    // A raster `data:image/*` is a legitimate image source that displays (never
    // executes) when navigated to, so an author-supplied inline image target
    // keeps its `link=self` anchor (the same exemption that lets an embedded
    // `data-uri` image link to itself).
    assert_eq!(
        render_macros("image:data:image/png;base64,iVBORw0KGgo=[alt,link=self]"),
        r#"<span class="image"><a class="image" href="data:image/png;base64,iVBORw0KGgo="><img src="data:image/png;base64,iVBORw0KGgo=" alt="alt"></a></span>"#
    );
}

#[test]
fn image_link_self_with_author_supplied_svg_data_uri_drops_the_wrapping_link() {
    // An SVG opened as a top-level document runs its embedded script, so an
    // author-supplied `data:image/svg+xml` target – which `link=self` would
    // promote into a followable `href` – is rejected, unlike a raster data URI
    // (above) or a trusted embedded SVG (which is resolved from a file target,
    // not passed through as a URI). Only the anchor is dropped. (The `<` `>`
    // reaching the `src` are escaped by the special-characters step.)
    assert_eq!(
        render_macros("image:data:image/svg+xml,<svg onload='alert(1)'></svg>[alt,link=self]"),
        // The space in the target is percent-encoded in the resolved `src`.
        r#"<span class="image"><img src="data:image/svg+xml,&lt;svg%20onload='alert(1)'&gt;&lt;/svg&gt;" alt="alt"></span>"#
    );
}

#[test]
fn image_link_self_with_author_supplied_svg_data_uri_records_a_warning() {
    let mut parser = Parser::default();
    let doc = parser.parse("image:data:image/svg+xml,<svg onload='alert(1)'></svg>[alt,link=self]");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnsafeLinkSchemeRejected(
            "data:image/svg+xml,&lt;svg onload='alert(1)'&gt;&lt;/svg&gt;".to_string()
        )
    );
}

// ---- Dangerous-scheme rejection (explicit `link:` macro) -------------------

#[test]
fn link_macro_neutralizes_javascript_scheme() {
    // A script URI cannot be made safe by escaping, so the macro is left as
    // inert source text instead of becoming a live link.
    assert_eq!(
        render_paragraph("link:javascript:alert(1)[click]"),
        "link:javascript:alert(1)[click]"
    );
}

#[test]
fn link_macro_scheme_rejection_is_case_insensitive() {
    assert_eq!(
        render_paragraph("link:JavaScript:alert(1)[click]"),
        "link:JavaScript:alert(1)[click]"
    );
}

#[test]
fn link_macro_neutralizes_data_scheme() {
    // The `<script>` is already escaped by the special-characters step, so the
    // literal text that survives is inert.
    assert_eq!(
        render_paragraph("link:data:text/html,<script>alert(1)</script>[click]"),
        "link:data:text/html,&lt;script&gt;alert(1)&lt;/script&gt;[click]"
    );
}

#[test]
fn link_macro_neutralizes_vbscript_scheme() {
    assert_eq!(
        render_paragraph("link:vbscript:msgbox(1)[click]"),
        "link:vbscript:msgbox(1)[click]"
    );
}

#[test]
fn link_macro_neutralizes_scheme_with_authority_separator() {
    assert_eq!(
        render_paragraph("link:javascript://alert(1)[click]"),
        "link:javascript://alert(1)[click]"
    );
}

// ---- Legitimate targets remain live links ----------------------------------

#[test]
fn link_macro_allows_relative_path() {
    assert_eq!(
        render_paragraph("link:/home.html[Home]"),
        r#"<a href="/home.html">Home</a>"#
    );
}

#[test]
fn link_macro_allows_https_scheme() {
    assert_eq!(
        render_paragraph("link:https://example.com[x]"),
        r#"<a href="https://example.com">x</a>"#
    );
}

#[test]
fn mailto_macro_is_not_rejected() {
    assert_eq!(
        render_paragraph("mailto:a@b.com[mail]"),
        r#"<a href="mailto:a@b.com">mail</a>"#
    );
}

#[test]
fn link_macro_allows_target_that_merely_starts_with_scheme_letters() {
    // A target whose leading characters coincide with a dangerous scheme name
    // but is not that scheme (no delimiting colon) must still render.
    assert_eq!(
        render_paragraph("link:javascriptfoo.html[x]"),
        r#"<a href="javascriptfoo.html">x</a>"#
    );
}

#[test]
fn link_macro_allows_nonstandard_scheme_that_is_not_dangerous() {
    assert_eq!(
        render_paragraph("link:mydata:thing[x]"),
        r#"<a href="mydata:thing">x</a>"#
    );
}

// ---- Rejection is reported as a warning ------------------------------------

#[test]
fn rejected_scheme_records_a_warning() {
    let mut parser = Parser::default();
    let doc = parser.parse("link:javascript:alert(1)[click]");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
    );
    assert_eq!(warnings[0].source.line(), 1);
}

#[test]
fn allowed_link_records_no_warning() {
    let mut parser = Parser::default();
    let doc = parser.parse("link:https://example.com[x]");

    assert_eq!(doc.warnings().count(), 0);
}
