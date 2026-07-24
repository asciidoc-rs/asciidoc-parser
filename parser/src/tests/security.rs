//! Regression tests for HTML-attribute-breakout and script-URI injection when
//! rendering untrusted AsciiDoc to HTML.
//!
//! These behaviors are a deliberate divergence from Ruby Asciidoctor (which
//! does neither the quote-escaping nor the scheme rejection exercised here), so
//! they live outside the `asciidoctor_rb` faithfulness suite.
//!
//! Two properties are covered:
//!
//!   * every `href`/`target` and the icon `class`/`title`/`width`/`height`
//!     attributes escape the `"` delimiter, so a stray quote cannot close the
//!     attribute and inject further markup; and
//!   * the explicit `link:` macro rejects targets whose scheme could execute
//!     script (`javascript:`, `data:`, `vbscript:`), rendering them as inert
//!     source text rather than a live link.
//!
//! The values reaching the renderer are already special-character-escaped
//! (`< > &`) by the substitution pipeline, so the tests also confirm that the
//! added escaping does not double-escape those characters.

use crate::{
    Parser,
    blocks::{Block, IsBlock, SimpleBlock},
    content::{Content, SubstitutionStep},
    parser::ModificationContext,
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
                walk(block.nested_blocks())
            }
        })
    }

    walk(doc.nested_blocks())
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
