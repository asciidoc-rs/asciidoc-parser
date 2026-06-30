use std::{borrow::Cow, path::Path, sync::LazyLock};

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use regex::{Captures, Regex, Replacer};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, content::XrefSegment},
    parser::{IconRenderParams, ImageRenderParams, LinkRenderParams, LinkRenderType},
};

pub(super) fn apply_macros(content: &mut Content<'_>, parser: &Parser) {
    let /* mut */ text = content.rendered().to_string();
    let found_square_bracket = text.contains('[');
    let found_colon = text.contains(':');
    let found_macroish = found_square_bracket && found_colon;
    // let found_macroish_short = found_macroish && text.contains(":[");

    // Bibliography anchors (`[[[id]]]` / `[[[id,xreftext]]]`) are recognized only
    // in the principal text of a bibliography list item; the parser sets a flag
    // while substituting that text. This runs before the inline-anchor pass below
    // so a bibliography anchor is consumed as a whole rather than being mistaken
    // for a regular inline anchor (`[[id]]`) wrapped in square brackets.
    if found_square_bracket && text.contains("[[[") && parser.in_bibliography_list_item.get() {
        let replacer = InlineBiblioAnchorReplacer {
            parser,
            source: content.original(),
        };

        if let Cow::Owned(new_result) =
            INLINE_BIBLIO_ANCHOR.replace_all(content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    // TO DO (#262): Implement extensions that can define macros.
    // Port Ruby Asciidoctor's implementation from
    // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb#L306-L347.

    // TO DO (#263): Implement `kbd:` and `btn:` macros.
    // Port Ruby Asciidoctor's implementation from
    // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb#L349-L377.

    if found_macroish && (text.contains("image:") || text.contains("icon:")) {
        let replacer = InlineImageMacroReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_IMAGE_MACRO.replace_all(content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    /*
    if (text.contains("((") && text.contains("))"))
        || (found_macroish_short && text.contains("dexterm"))
    {
        todo!("Index term macro");
        // Port Ruby Asciidoctor's implementation from lines 439..536.
    }
    */

    if found_colon && text.contains("://") {
        let replacer = InlineLinkReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_LINK.replace_all(content.rendered(), replacer) {
            content.rendered = new_result.into();
        }
    }

    if found_macroish && (text.contains("link:") || text.contains("ilto:")) {
        let replacer = InlineLinkMacroReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_LINK_MACRO.replace_all(content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    if text.contains('@') {
        let replacer = InlineEmailReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_EMAIL.replace_all(content.rendered(), replacer) {
            content.rendered = new_result.into();
        }
    }

    if (found_square_bracket && text.contains("[[")) || (found_macroish && text.contains("or:")) {
        let replacer = InlineAnchorReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_ANCHOR.replace_all(content.rendered(), replacer) {
            content.rendered = new_result.into();
        }
    }

    // Cross-references (`<<id>>`, `<<id,text>>`, `xref:id[]`, `xref:id[text]`).
    //
    // By the time the macros step runs, the special-characters step has already
    // turned `<<` / `>>` into `&lt;&lt;` / `&gt;&gt;`. We do NOT resolve the
    // reference here, because its target may be defined later in the document
    // (or, for multi-document workflows, in another document). Instead each
    // cross-reference is recorded as a deferred `XrefSegment` and a placeholder
    // is left in the rendered text; resolution happens later via
    // `Document::resolve_references`.
    if text.contains("&lt;&lt;") || (found_macroish && text.contains("xref:")) {
        let mut xrefs: Vec<XrefSegment> = vec![];

        let replaced = match INLINE_XREF
            .replace_all(content.rendered(), InlineXrefReplacer { xrefs: &mut xrefs })
        {
            Cow::Owned(new_result) => Some(new_result),
            Cow::Borrowed(_) => None,
        };

        if let Some(new_result) = replaced {
            content.rendered = new_result.into();
        }

        content.set_deferred_xrefs(xrefs);
    }

    /*
    if found_macroish && text.contains("tnote") {
        todo!("Port footnote macro");
        // Port Ruby Asciidoctor's implementation from lines 842..884.
    }
    */
}

static INLINE_IMAGE_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                    
            \\?                         # Optional escape: literal backslash
            i(?:mage|con):              # 'image:' or 'icon:' prefix

            (                           # Group 1: the target
                [^:\s\[\n]                  # First char: not colon, whitespace, [, or newline
                [^\[\n]*?                   # Middle chars: any except [ or newline, lazily
                [^\s\[\n]                   # Last char: not whitespace, [, or newline
            )?                          # Entire target group is optional

            \[                          # Opening square bracket

            (                           # Group 2: bracketed text
                |                       #   EITHER: empty alt text
                .*?[^\\]                #   OR: content ending in a non-backslash
            )

            \]                          # Closing square bracket
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineImageMacroReplacer<'p>(&'p Parser);

impl Replacer for InlineImageMacroReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps[0].starts_with('\\') {
            // Honor the escape.
            dest.push_str(&caps[0][1..]);
            return;
        }

        let target = &caps[1];
        let span = Span::new(&caps[2]);
        let attrlist = Attrlist::parse(span, self.0, AttrlistContext::Inline)
            .item
            .item;

        let default_alt = basename(&target.replace(['_', '-'], " "));
        // IMPORTANT: Implementations of `render_icon` and `render_image` need to
        // remember to use `default_alt` when attrlist doesn't contain a value for
        // `alt`.

        if caps[0].starts_with("image:") {
            // TO DO: Register image with parser?
            // IMPORTANT: May require interior mutability on Parser because it looks like we
            // can't pass mutable references to Parser in a recursive Regex replacement.

            // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/335):
            // todo!("Port this: {}", "doc.register :images, target");

            let params = ImageRenderParams {
                target,
                alt: attrlist
                    .named_or_positional_attribute("alt", 1)
                    .map_or(default_alt, |a| {
                        normalize_text_lf_escaped_bracket(a.value())
                    }),
                width: attrlist
                    .named_or_positional_attribute("width", 2)
                    .map(|a| a.value()),
                height: attrlist
                    .named_or_positional_attribute("height", 3)
                    .map(|a| a.value()),
                attrlist: &attrlist,
                parser: self.0,
            };

            self.0.renderer.render_image(&params, dest);
        } else {
            let params = IconRenderParams {
                target,
                alt: attrlist.named_attribute("alt").map_or(default_alt, |a| {
                    normalize_text_lf_escaped_bracket(a.value())
                }),
                size: attrlist
                    .named_or_positional_attribute("size", 1)
                    .map(|a| a.value()),
                attrlist: &attrlist,
                parser: self.0,
            };

            self.0.renderer.render_icon(&params, dest);
        }
    }
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn normalize_text_lf_escaped_bracket(text: &str) -> String {
    text.replace("\n", " ").replace("\\]", "]")
}

static INLINE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?msx)
        ( ^ | link: | [\ \t] | \\?&lt;() | [>\(\)\[\];"'] )   # capture group 1: prefix
                                                              # capture group 2: flag for prefix == "&lt;"
        ( \\? (?: https? | file | ftp | irc ):// )            # capture group 3: scheme
        (?:
            ( [^\s\[\]]+ )                                    # capture group 4: target
            \[ ( | .*?[^\\] ) \]                              # capture group 5: attrlist
          | ( [^\s]+? ) &gt;                                  # capture group 6: URL inside <>
                                                              # (Ruby gates this with a `\2` back-ref to
                                                              # group 2; unsupported here - see issue #503)
          | ( [^\s\[\]<]* ( [^\s,.?!\[\]<\)] ) )              # capture group 7: bare link,
                                                              # capture group 8: trailing char
        )
    "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineLinkReplacer<'p>(&'p Parser);

impl Replacer for InlineLinkReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        let mut attrlist = Attrlist::parse(Span::default(), self.0, AttrlistContext::Inline)
            .item
            .item;

        if caps.get(2).is_some() && caps.get(5).is_none() {
            // Honor the escapes.
            if caps[1].starts_with('\\') {
                dest.push_str(&caps[0][1..]);
                return;
            }

            if caps[3].starts_with('\\') {
                dest.push_str(&caps[1]);
                dest.push_str(&caps[0][caps[1].len() + 1..]);
                return;
            }

            let Some(link_suffix) = caps.get(6) else {
                dest.push_str(&caps[0]);
                return;
            };

            let target = format!(
                "{scheme}{link_suffix}",
                scheme = &caps[3],
                link_suffix = link_suffix.as_str()
            );

            // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/335):
            // doc.register :links, target

            let link_text = if self.0.is_attribute_set("hide-uri-scheme") {
                URI_SNIFF.replace_all(&target, "").into_owned()
            } else {
                target.clone()
            };

            let params = LinkRenderParams {
                target,
                link_text,
                extra_roles: vec!["bare"],
                window: None,
                type_: LinkRenderType::Link,
                attrlist: &attrlist,
                parser: self.0,
            };

            self.0.renderer.render_link(&params, dest);

            return;
        }

        let mut prefix = caps[1].to_string();
        let scheme = &caps[3];

        // Honor the escape.
        if scheme.starts_with('\\') {
            dest.push_str(&prefix);
            dest.push_str(&caps[0][prefix.len() + 1..]);
            return;
        }

        // Groups 4, 6, and 7 are mutually exclusive regex alternatives;
        // exactly one will be Some(_) when we reach this point.
        // Group 4 = formal macro target (URL before '['), group 7 = bare link.
        //
        // Group 6 is the URL captured before a `&gt;`. When the `&lt;` prefix is
        // also present (group 2), the angle-bracketed-URL case is handled earlier
        // and returns. Reaching here with group 6 means a stray `&gt;` with no
        // matching `&lt;`; Asciidoctor treats that as a bare link that keeps the
        // literal `&gt;`, then strips trailing punctuation via the rule below
        // (e.g. `https://example.org>` renders with href `https://example.org&gt`
        // and the `;` left outside the link).
        //
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/503):
        // Group 6 should never participate without group 2. Ruby gates it with a
        // `\2` back-reference, which the `regex` crate can't express, so it can
        // fire spuriously here and a stray `&gt;` followed by more punctuation
        // (e.g. `>;`) still diverges from Asciidoctor.
        let url_part = caps
            .get(4)
            .map(|m| m.as_str().to_owned())
            .or_else(|| caps.get(7).map(|m| m.as_str().to_owned()))
            .or_else(|| caps.get(6).map(|m| format!("{}&gt;", m.as_str())))
            .unwrap_or_default();
        let mut target = format!("{scheme}{url_part}");

        let mut suffix = "".to_owned();
        let mut link_text: Option<String> = None;

        // NOTE: If capture group 5 exists (the attrlist), we're looking at a formal macro (e.g., https://example.org[]).
        if let Some(attrlist) = caps.get(5) {
            if prefix == "link:" {
                prefix = "".to_owned();
            }

            if !attrlist.is_empty() {
                link_text = Some(attrlist.as_str().to_owned());
            }
        } else {
            if prefix == "link" || prefix == "\"" || prefix == "'" {
                // Note from the Ruby implementation which also applies to this if clause:

                // Invalid macro syntax (link: prefix w/o trailing square brackets or URL
                // enclosed in quotes).

                // FIXME: We probably shouldn't even get here when the link: prefix is present.
                // The regex is doing too much.
                dest.push_str(&caps[0]);
                return;
            }

            // Strip a trailing ';' or ':' (and an adjacent ')') out of a bare
            // URL. Keying off the target's final character rather than capture
            // group 8 covers both the bare-link case (group 7) and the stray
            // `&gt;` case (group 6, whose reconstructed URL ends in ';').
            if let Some(tail) = target.chars().last().filter(|c| *c == ';' || *c == ':') {
                target.truncate(target.len() - 1);
                suffix = tail.to_string();

                if target.ends_with(')') {
                    target.truncate(target.len() - 1);
                    suffix = format!("){suffix}");
                }
            }
        }

        let mut bare = false;

        let link_text_for_attrlist = link_text.clone().unwrap_or_default();
        let span_for_attrlist = Span::new(&link_text_for_attrlist);
        let mut window: Option<&'static str> = None;

        let link_text = if let Some(mut link_text) = link_text {
            link_text = link_text.replace("\\]", "]");

            if link_text.contains('=') {
                let (lt, attrs) = extract_attributes_from_text(&span_for_attrlist, self.0, None);

                link_text = lt.replace("\\\"", "\"");
                attrlist = attrs; // ???
            }

            if link_text.ends_with('^') {
                link_text.truncate(link_text.len() - 1);
                window = Some("_blank");
            }

            if link_text.is_empty() {
                bare = true;

                if self.0.is_attribute_set("hide-uri-scheme") {
                    // NOTE: The modified target will not be a bare URI scheme (e.g., http://) in this case.
                    URI_SNIFF.replace_all(&target, "").into_owned()
                } else {
                    target.clone()
                }
            } else {
                link_text
            }
        } else {
            // NOTE: The modified target will not be a bare URI scheme (e.g., http://) in this case.
            bare = true;

            if self.0.is_attribute_set("hide-uri-scheme") {
                URI_SNIFF.replace_all(&target, "").into_owned()
            } else {
                target.clone()
            }
        };

        let extra_roles = if bare { vec!["bare"] } else { vec![] };

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/335):
        // doc.register :links, (link_opts[:target] = target)

        dest.push_str(&prefix);

        let params = LinkRenderParams {
            target,
            link_text,
            extra_roles,
            window,
            type_: LinkRenderType::Link,
            attrlist: &attrlist,
            parser: self.0,
        };

        self.0.renderer.render_link(&params, dest);

        dest.push_str(&suffix);
    }
}

static INLINE_LINK_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                # (?x) extended mode, (?s) dot matches newline

        \\?                     # Optional backslash escape before macro

        (?:                     # Non-capturing group for macro name
            link                #   'link'
          | (mailto)            #   capture group 1: 'mailto'
        )

        :                       # Colon after macro name

        (?:                     # Non-capturing outer group
            ().                 #   capture group 2: empty target
          | ([^:\s\[] [^\s\[]*) #   capture group 3: valid target (no colon/space/'[')
        )

        \[                      # Opening square bracket

        (?:                     # Non-capturing outer group
            ()                  #   capture group 4: empty label
          | (.*?[^\\])          #   capture group 5: minimally match anything, not ending in '\'
        )

        \]                      # Closing square bracket
    "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineLinkMacroReplacer<'p>(&'p Parser);

impl Replacer for InlineLinkMacroReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps[0].starts_with('\\') {
            // Honor the escape.
            dest.push_str(&caps[0][1..]);
            return;
        }

        let (mailto, mailto_text, mut target) = if caps.get(1).is_some() {
            let mailto_text = &caps[3];
            (
                caps.get(1).map(|c| c.as_str()),
                Some(mailto_text),
                format!("mailto:{mailto_text}"),
            )
        } else {
            (None, None, caps[3].to_string())
        };

        let mut attrlist: Option<Attrlist<'_>> = None;
        let link_type = LinkRenderType::Link;

        let mut link_text = caps
            .get(5)
            .map(|c| c.as_str().to_string())
            .unwrap_or_default();

        let link_text_for_attrlist = link_text.replace("\n", " ");
        let span_for_attrlist = Span::new(&link_text_for_attrlist);
        let mut window: Option<&'static str> = None;

        if !link_text.is_empty() {
            link_text = link_text.replace("\\]", "]");

            if let Some(_mailto) = mailto {
                if link_text.contains(',') {
                    let (lt, attrs) =
                        extract_attributes_from_text(&span_for_attrlist, self.0, None);

                    link_text = lt;

                    if let Some(target_attr) = attrs.nth_attribute(2) {
                        target = format!(
                            "{target}?subject={subject}",
                            subject = encode_uri_component(target_attr.value())
                        );

                        if let Some(body) = attrs.nth_attribute(3) {
                            target = format!(
                                "{target}&amp;body={body}",
                                body = encode_uri_component(body.value())
                            );
                        }
                    }

                    attrlist = Some(attrs);
                }
            } else if link_text.contains('=') {
                let (lt, attrs) = extract_attributes_from_text(&span_for_attrlist, self.0, None);
                link_text = lt;

                attrlist = Some(attrs);
            }

            if link_text.ends_with('^') {
                link_text.truncate(link_text.len() - 1);
                window = Some("_blank");
            }
        }

        let attrlist = if let Some(attrlist) = attrlist {
            attrlist
        } else {
            Attrlist::parse(Span::default(), self.0, AttrlistContext::Inline)
                .item
                .item
        };

        let mut extra_roles: Vec<&str> = vec![];

        if link_text.is_empty() {
            // mailto is a special case; already processed.
            if let Some(_mailto) = mailto {
                link_text = mailto_text.map(|s| s.to_owned()).unwrap_or_default();
            } else {
                link_text = if self.0.is_attribute_set("hide-uri-scheme") {
                    let lt = URI_SNIFF.replace_all(&target, "").into_owned();
                    if lt.is_empty() { target.clone() } else { lt }
                } else {
                    target.clone()
                };

                extra_roles.push("bare");
            }
        }

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/335):
        // doc.register :links, (link_opts[:target] = target)

        let params = LinkRenderParams {
            target,
            link_text: link_text.clone(),
            extra_roles,
            window,
            type_: link_type,
            attrlist: &attrlist,
            parser: self.0,
        };

        self.0.renderer.render_link(&params, dest);
    }
}

/// This function is used in cases when the attrlist can be mixed with the text
/// of a macro. If no attributes are detected aside from the first positional
/// attribute, and the first positional attribute matches the attrlist, then the
/// original text is returned.
///
/// Precondition: Any new-line characters (`\n`) must be replaced with spaces
/// prior to calling this function.
fn extract_attributes_from_text<'src>(
    text: &'src Span<'src>,
    parser: &Parser,
    default_text: Option<&str>,
) -> (String, Attrlist<'src>) {
    let attrlist_maw = Attrlist::parse(*text, parser, AttrlistContext::Inline);
    let attrs = attrlist_maw.item.item;

    if let Some(resolved_text) = attrs.nth_attribute(1) {
        // NOTE: If resolved text remains unchanged, return an empty attribute list and
        // return unparsed text. Commented out because I haven't seen an example of this
        // happening in practice. Each of the call sites for this function introduces a
        // constraint that should make this impossible.

        /* if resolved_text.value() == text.data() {
            let empty_attrs = Attrlist::parse(Span::default(), parser, AttrlistContext::Inline).item.item;
            (text.data().to_owned(), empty_attrs)
        } else { */
        (resolved_text.value().to_owned(), attrs)
        /* } */
    } else {
        let default_text = default_text.map(|s| s.to_string());
        (default_text.unwrap_or_default(), attrs)
    }
}

// Ruby CGI.escape allows A-Z a-z 0-9 *_.-
// It encodes space as '+'. (We'll fix afterward.)
// Start with the standard URL encoding set.
const CGI_ESCAPE_SET: &AsciiSet = &CONTROLS
    .add(b' ') // space
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'+') // plus must be escaped
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

fn encode_uri_component(s: &str) -> String {
    // First escape with percent-encoding.
    let encoded = utf8_percent_encode(s, CGI_ESCAPE_SET).to_string();

    // Then apply the Ruby `.gsub('+', '%20')` logic.
    // But note: percent-encoding gives us "%20" for space already,
    // so we need to manually *introduce* '+' for space first,
    // then swap them out.
    let with_plus = encoded.replace("%20", "+");
    with_plus.replace('+', "%20")
}

/// Matches an inline e-mail address.
///
/// # Example
/// `doc.writer@example.com`
static INLINE_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)                         # verbose mode (ignore whitespace & comments)

        ([\\>:/]?)                      # capture group 1: prefix that causes mismatch: \, >, :, or /

        (                               # capture group 2: actual e-mail address
            [\w_]                           # leading word character
            (?: &amp; | [\w\-.%+] )*        # subsequent word chars or symbols (&amp;, ., -, %, +)
            @                               # at sign
            [\p{L}\p{Nd}]                   # leading letter or digit in domain
            [\p{L}\p{Nd}_\-.]*              # rest of domain
            \.[a-zA-Z]{2,5}                 # dot + TLD (2–5 ASCII letters)
        )

        \b                              # word boundary
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineEmailReplacer<'p>(&'p Parser);

impl Replacer for InlineEmailReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if let Some(escape) = &caps.get(1)
            && !escape.is_empty()
        {
            if escape.as_str() == "\\" {
                dest.push_str(&caps[0][1..]);
            } else {
                dest.push_str(&caps[0]);
            }
            return;
        }

        let target = format!("mailto:{mailto}", mailto = &caps[2]);

        let attrlist = Attrlist::parse(Span::default(), self.0, AttrlistContext::Inline)
            .item
            .item;

        let params = LinkRenderParams {
            target: target.clone(),
            link_text: caps[2].to_owned(),
            extra_roles: vec![],
            window: None,
            type_: LinkRenderType::Link,
            attrlist: &attrlist,
            parser: self.0,
        };

        self.0.renderer.render_link(&params, dest);
    }
}

/// Matches a bibliography anchor at the start of a bibliography list item.
///
/// The label must be _non-numeric_ (it may contain digits, but must not begin
/// with one), so a list item that opens with something like `[[[1984]]]` is
/// left untouched. An optional xreftext follows a comma.
///
/// ## Examples
///
/// * `[[[label]]]`
/// * `[[[label,xreftext]]]`
static INLINE_BIBLIO_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
        (\\)?                           # (1) optional escape backslash
        \[\[\[                          # opening triple bracket
          (                             # (2) bibliography label
            [\p{Alphabetic}_:]              # first char: letter, '_' or ':' (never a digit)
            [\p{Alphabetic}\p{Nd}_\-:.]*    # rest: letters/digits/_/-/:/.
          )
          (?: , \s* (.+?) )?            # (3) optional xreftext after a comma
        \]\]\]                          # closing triple bracket
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineBiblioAnchorReplacer<'p, 's> {
    parser: &'p Parser,

    /// The original (pre-substitution) span of the content being rendered, used
    /// to locate a duplicate-id warning.
    source: Span<'s>,
}

impl Replacer for InlineBiblioAnchorReplacer<'_, '_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps.get(1).is_some() {
            // Honor the escape: emit the anchor literally (sans backslash).
            dest.push_str(&caps[0][1..]);
            return;
        }

        let id = &caps[2];

        // The displayed reference text is the xreftext if supplied, otherwise the
        // label itself, always enclosed in square brackets (e.g. `[gof]`). This
        // same bracketed text is registered as the entry's reftext so a
        // cross-reference to the entry renders identically.
        let label = caps.get(3).map(|m| m.as_str()).unwrap_or(id);
        let reftext = format!("[{label}]");

        if self
            .parser
            .register_ref(id, Some(&reftext), crate::document::RefType::Bibliography)
            .is_err()
        {
            self.parser.record_substitution_warning(
                self.source,
                crate::warnings::WarningType::DuplicateId(id.to_string()),
            );
        }

        self.parser.renderer.render_anchor(id, None, dest);
        dest.push_str(&reftext);
    }
}

/// Matches an anchor (i.e., id + optional reference text) in the flow of text.
///
/// ##Examples
///
/// * `[[idname]]`
/// * `[[idname,Reference Text]]`
/// * `anchor:idname[]`
/// * `anchor:idname[Reference Text]`
static INLINE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
    (\\)?                           # (1) optional escape backslash before the anchor

    (?:                             # either [[id[, reftext]]] OR anchor:id[reftext]
      \[\[                          # [[
        (                           # (2) anchor id for [[...]]
          [\p{Alphabetic}_:]        #     first char: letter, '_' or ':'
          [\p{Alphabetic}\p{Nd}_\-:.]*  # rest: letters/digits/_ or '-', ':', '.'
        )
        (?: , \s* (.+?) )?          # (3) optional reftext after comma (lazy)
        \]\]                        # ]]
      |
        anchor:                     # 'anchor:' prefix
        (                           # (4) anchor id for anchor:...[]
          [\p{Alphabetic}_:]        #     first char: letter, '_' or ':'
          [\p{Alphabetic}\p{Nd}_\-:.]*  # rest: letters/digits/_ or '-', ':', '.'
        )                           # end (4)
        \[                          # opening '[' for reftext
          (?:                       # either empty [] or a non-empty reftext
            \]                      #   empty -> immediate ']'
          |                         #   OR
            (.*?[^\\])              # (5) non-empty reftext (ends with a non-escaped char)
            \]                      #   closing ']'
          )
    )                               # end alternation
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineAnchorReplacer<'p>(&'p Parser);

impl Replacer for InlineAnchorReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps.get(1).is_some() {
            dest.push_str(&caps[0][1..]);
            return;
        }

        // NOTE: reftext is only relevant for DocBook output;
        // in that case it is used as value of xreflabel attribute.

        let (id, reftext) = if let Some(id) = caps.get(2) {
            (id.as_str(), caps.get(3).map(|m| m.as_str().to_string()))
        } else {
            (
                &caps[4],
                caps.get(5)
                    .map(|m| m.as_str().to_string().replace("\\]", "]")),
            )
        };

        // Register the inline anchor so that later cross-references can resolve
        // against it. A duplicate ID here is non-fatal (first registration
        // wins); block- and section-level registration paths surface duplicate
        // warnings, so we don't double-report them for inline anchors.
        let _ = self
            .0
            .register_ref(id, reftext.as_deref(), crate::document::RefType::Anchor);

        self.0.renderer.render_anchor(id, reftext, dest);
    }
}

/// Matches a cross-reference, in either the double-angle-bracket shorthand or
/// the `xref:` macro form.
///
/// Note that the special-characters substitution runs before macros, so by this
/// point `<<` and `>>` have already become `&lt;&lt;` and `&gt;&gt;`.
///
/// ## Examples
///
/// * `<<idname>>` (seen here as `&lt;&lt;idname&gt;&gt;`)
/// * `<<idname,Reference Text>>`
/// * `xref:idname[]`
/// * `xref:idname[Reference Text]`
static INLINE_XREF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
        (\\)?                           # (1) optional escape backslash
        (?:
            &lt;&lt;                     #   shorthand: << (post special-chars)
              ( .*? )                    # (2) refid plus optional ", reftext"
            &gt;&gt;                     #   >>
          |
            xref:                        #   'xref:' macro form
              ( [^:\s\[] [^\s\[]* )      # (3) target
            \[                           #   opening '['
              ( | .*?[^\\] )             # (4) reftext: empty or ends non-escaped
            \]                           #   closing ']'
        )
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineXrefReplacer<'x> {
    /// Accumulates the cross-references discovered during replacement, in the
    /// same order as the placeholders emitted into the output.
    xrefs: &'x mut Vec<XrefSegment>,
}

impl Replacer for InlineXrefReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps.get(1).is_some() {
            // Honor the escape: emit the reference literally (sans backslash).
            dest.push_str(&caps[0][1..]);
            return;
        }

        let (target, provided_text) = if let Some(inner) = caps.get(2) {
            // Shorthand form: split an optional ", reftext" off the id.
            match inner.as_str().split_once(',') {
                Some((id, text)) => (id.trim().to_string(), Some(text.trim().to_string())),
                None => (inner.as_str().trim().to_string(), None),
            }
        } else {
            let target = caps[3].to_string();
            let text = caps
                .get(4)
                .map(|m| m.as_str().to_string())
                .filter(|s| !s.is_empty());
            (target, text)
        };

        let index = self.xrefs.len();
        self.xrefs.push(XrefSegment {
            target,
            provided_text,
            resolved: None,
        });

        dest.push_str(&Content::xref_placeholder(index));
    }
}

static URI_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"^\p{alpha}[\p{alpha}\p{digit}.+-]+:/{0,2}"#).unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    //! This test suite fills in a few coverage gaps after doing spec-driven
    //! development (SDD) for macro parsing.

    mod inline_link {
        use crate::tests::prelude::*;

        #[test]
        fn escape_angle_bracket_autolink_before_lt() {
            let doc = Parser::default()
                .parse("You'll often see \\<https://example.org> used in examples.");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You'll often see \\<https://example.org> used in examples.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "You&#8217;ll often see &lt;https://example.org&gt; used in examples.",
                        },
                        source: Span {
                            data: "You'll often see \\<https://example.org> used in examples.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "You'll often see \\<https://example.org> used in examples.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn escape_angle_bracket_autolink_before_scheme() {
            let doc = Parser::default()
                .parse("You'll often see <\\https://example.org> used in examples.");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You'll often see <\\https://example.org> used in examples.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "You&#8217;ll often see &lt;https://example.org&gt; used in examples.",
                        },
                        source: Span {
                            data: "You'll often see <\\https://example.org> used in examples.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "You'll often see <\\https://example.org> used in examples.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn empty_inside_angle_brackets() {
            let doc = Parser::default().parse("There's no actual link <https://> in here.");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "There's no actual link <https://> in here.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "There&#8217;s no actual link &lt;https://&gt; in here.",
                        },
                        source: Span {
                            data: "There's no actual link <https://> in here.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "There's no actual link <https://> in here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn hide_uri_scheme() {
            let doc = Parser::default().parse("= Test Page\n:hide-uri-scheme:\n\nWe don't want you to know that this is HTTP: <https://example.com> just now.");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: Some(Span {
                            data: "Test Page",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },),
                        title: Some("Test Page",),
                        attributes: &[Attribute {
                            name: Span {
                                data: "hide-uri-scheme",
                                line: 2,
                                col: 2,
                                offset: 13,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":hide-uri-scheme:",
                                line: 2,
                                col: 1,
                                offset: 12,
                            },
                        },],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "= Test Page\n:hide-uri-scheme:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "We don't want you to know that this is HTTP: <https://example.com> just now.",
                                line: 4,
                                col: 1,
                                offset: 31,
                            },
                            rendered: "We don&#8217;t want you to know that this is HTTP: <a href=\"https://example.com\" class=\"bare\">example.com</a> just now.",
                        },
                        source: Span {
                            data: "We don't want you to know that this is HTTP: <https://example.com> just now.",
                            line: 4,
                            col: 1,
                            offset: 31,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "= Test Page\n:hide-uri-scheme:\n\nWe don't want you to know that this is HTTP: <https://example.com> just now.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn link_with_semicolon_suffix() {
            let doc = Parser::default().parse(
                "You shouldn't visit https://example.com; it's just there to illustrate examples.",
            );

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You shouldn't visit https://example.com; it's just there to illustrate examples.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "You shouldn&#8217;t visit <a href=\"https://example.com\" class=\"bare\">https://example.com</a>; it&#8217;s just there to illustrate examples.",
                        },
                        source: Span {
                            data: "You shouldn't visit https://example.com; it's just there to illustrate examples.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "You shouldn't visit https://example.com; it's just there to illustrate examples.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn link_with_paren_and_colon_suffix() {
            let doc = Parser::default().parse(
            "You shouldn't visit that site (https://example.com): it's just there to illustrate examples.",
        );

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You shouldn't visit that site (https://example.com): it's just there to illustrate examples.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "You shouldn&#8217;t visit that site (<a href=\"https://example.com\" class=\"bare\">https://example.com</a>): it&#8217;s just there to illustrate examples.",
                        },
                        source: Span {
                            data: "You shouldn't visit that site (https://example.com): it's just there to illustrate examples.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "You shouldn't visit that site (https://example.com): it's just there to illustrate examples.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn named_attributes_without_link_text_and_hide_uri_scheme() {
            let doc = Parser::default()
            .parse("= Test\n:hide-uri-scheme:\n\nhttps://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: Some(Span {
                            data: "Test",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },),
                        title: Some("Test",),
                        attributes: &[Attribute {
                            name: Span {
                                data: "hide-uri-scheme",
                                line: 2,
                                col: 2,
                                offset: 8,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":hide-uri-scheme:",
                                line: 2,
                                col: 1,
                                offset: 7,
                            },
                        },],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "= Test\n:hide-uri-scheme:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
                                line: 4,
                                col: 1,
                                offset: 26,
                            },
                            rendered: "<a href=\"https://chat.asciidoc.org\" class=\"bare button\" target=\"_blank\" rel=\"nofollow\" noopener>chat.asciidoc.org</a>",
                        },
                        source: Span {
                            data: "https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
                            line: 4,
                            col: 1,
                            offset: 26,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "= Test\n:hide-uri-scheme:\n\nhttps://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }
    }

    mod link_macro {
        use crate::tests::prelude::*;

        #[test]
        fn escape_link_macro() {
            let doc =
                Parser::default().parse("A link macro looks like this: \\link:target[link text].");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "A link macro looks like this: \\link:target[link text].",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "A link macro looks like this: link:target[link text].",
                        },
                        source: Span {
                            data: "A link macro looks like this: \\link:target[link text].",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "A link macro looks like this: \\link:target[link text].",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn empty_mailto_link() {
            let doc = Parser::default().parse("mailto:[,Subscribe me]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "mailto:[,Subscribe me]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "mailto:[,Subscribe me]",
                        },
                        source: Span {
                            data: "mailto:[,Subscribe me]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "mailto:[,Subscribe me]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn empty_link_text_with_hide_uri_scheme() {
            let doc = Parser::default()
                .parse("= Test Document\n:hide-uri-scheme:\n\nlink:https://example.com[]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: Some(Span {
                            data: "Test Document",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },),
                        title: Some("Test Document",),
                        attributes: &[Attribute {
                            name: Span {
                                data: "hide-uri-scheme",
                                line: 2,
                                col: 2,
                                offset: 17,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":hide-uri-scheme:",
                                line: 2,
                                col: 1,
                                offset: 16,
                            },
                        },],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "= Test Document\n:hide-uri-scheme:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "link:https://example.com[]",
                                line: 4,
                                col: 1,
                                offset: 35,
                            },
                            rendered: "<a href=\"https://example.com\" class=\"bare\">example.com</a>",
                        },
                        source: Span {
                            data: "link:https://example.com[]",
                            line: 4,
                            col: 1,
                            offset: 35,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "= Test Document\n:hide-uri-scheme:\n\nlink:https://example.com[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn empty_mailto_link_text_with_hide_uri_scheme() {
            let doc = Parser::default()
                .parse("= Test Document\n:hide-uri-scheme:\n\nlink:mailto:fred@example.com[]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: Some(Span {
                            data: "Test Document",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },),
                        title: Some("Test Document",),
                        attributes: &[Attribute {
                            name: Span {
                                data: "hide-uri-scheme",
                                line: 2,
                                col: 2,
                                offset: 17,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":hide-uri-scheme:",
                                line: 2,
                                col: 1,
                                offset: 16,
                            },
                        },],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "= Test Document\n:hide-uri-scheme:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "link:mailto:fred@example.com[]",
                                line: 4,
                                col: 1,
                                offset: 35,
                            },
                            rendered: "<a href=\"mailto:fred@example.com\" class=\"bare\">fred@example.com</a>",
                        },
                        source: Span {
                            data: "link:mailto:fred@example.com[]",
                            line: 4,
                            col: 1,
                            offset: 35,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "= Test Document\n:hide-uri-scheme:\n\nlink:mailto:fred@example.com[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }
    }

    mod inline_anchor {
        use crate::tests::prelude::*;

        #[test]
        fn inline_ref_double_brackets() {
            let doc = Parser::default().parse("Here you can read about tigers.[[tigers]]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.[[tigers]]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.<a id=\"tigers\"></a>",
                        },
                        source: Span {
                            data: "Here you can read about tigers.[[tigers]]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.[[tigers]]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            "tigers",
                            RefEntry {
                                id: "tigers",
                                reftext: None,
                                ref_type: crate::document::RefType::Anchor,
                            },
                        )]),
                        reftext_to_id: HashMap::new(),
                    },
                }
            );
        }

        #[test]
        fn inline_ref_macro() {
            let doc = Parser::default().parse("Here you can read about tigers.anchor:tigers[]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.anchor:tigers[]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.<a id=\"tigers\"></a>",
                        },
                        source: Span {
                            data: "Here you can read about tigers.anchor:tigers[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.anchor:tigers[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            "tigers",
                            RefEntry {
                                id: "tigers",
                                reftext: None,
                                ref_type: crate::document::RefType::Anchor,
                            },
                        )]),
                        reftext_to_id: HashMap::new(),
                    },
                }
            );
        }

        #[test]
        fn inline_ref_with_reftext_double_brackets() {
            let doc = Parser::default().parse("Here you can read about tigers.[[tigers,Tigers]]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.[[tigers,Tigers]]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.<a id=\"tigers\"></a>",
                        },
                        source: Span {
                            data: "Here you can read about tigers.[[tigers,Tigers]]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.[[tigers,Tigers]]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            "tigers",
                            RefEntry {
                                id: "tigers",
                                reftext: Some("Tigers"),
                                ref_type: crate::document::RefType::Anchor,
                            },
                        )]),
                        reftext_to_id: HashMap::from([("Tigers", "tigers")]),
                    },
                }
            );
        }

        #[test]
        fn inline_ref_with_reftext_macro() {
            let doc =
                Parser::default().parse("Here you can read about tigers.anchor:tigers[Tigers]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.anchor:tigers[Tigers]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.<a id=\"tigers\"></a>",
                        },
                        source: Span {
                            data: "Here you can read about tigers.anchor:tigers[Tigers]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.anchor:tigers[Tigers]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            "tigers",
                            RefEntry {
                                id: "tigers",
                                reftext: Some("Tigers"),
                                ref_type: crate::document::RefType::Anchor,
                            },
                        )]),
                        reftext_to_id: HashMap::from([("Tigers", "tigers")]),
                    },
                }
            );
        }

        #[test]
        fn mixed_inline_anchor_macro_and_anchor_shorthand_with_empty_reftext() {
            let doc =
                Parser::default().parse("anchor:one[][[two]]anchor:three[][[four]]anchor:five[]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "anchor:one[][[two]]anchor:three[][[four]]anchor:five[]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: r#"<a id="one"></a><a id="two"></a><a id="three"></a><a id="four"></a><a id="five"></a>"#,
                        },
                        source: Span {
                            data: "anchor:one[][[two]]anchor:three[][[four]]anchor:five[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "anchor:one[][[two]]anchor:three[][[four]]anchor:five[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([
                            (
                                "one",
                                RefEntry {
                                    id: "one",
                                    reftext: None,
                                    ref_type: crate::document::RefType::Anchor,
                                },
                            ),
                            (
                                "two",
                                RefEntry {
                                    id: "two",
                                    reftext: None,
                                    ref_type: crate::document::RefType::Anchor,
                                },
                            ),
                            (
                                "three",
                                RefEntry {
                                    id: "three",
                                    reftext: None,
                                    ref_type: crate::document::RefType::Anchor,
                                },
                            ),
                            (
                                "four",
                                RefEntry {
                                    id: "four",
                                    reftext: None,
                                    ref_type: crate::document::RefType::Anchor,
                                },
                            ),
                            (
                                "five",
                                RefEntry {
                                    id: "five",
                                    reftext: None,
                                    ref_type: crate::document::RefType::Anchor,
                                },
                            ),
                        ]),
                        reftext_to_id: HashMap::new(),
                    },
                }
            );
        }

        #[test]
        fn inline_ref_can_start_with_colon() {
            let doc = Parser::default().parse("[[:idname]] text");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "[[:idname]] text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "<a id=\":idname\"></a> text",
                        },
                        source: Span {
                            data: "[[:idname]] text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "[[:idname]] text",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            ":idname",
                            RefEntry {
                                id: ":idname",
                                reftext: None,
                                ref_type: crate::document::RefType::Anchor,
                            },
                        )]),
                        reftext_to_id: HashMap::new(),
                    },
                }
            );
        }

        #[test]
        fn inline_ref_cannot_start_with_digit() {
            let doc = Parser::default().parse("[[1-install]] text");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "[[1-install]] text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "[[1-install]] text",
                        },
                        source: Span {
                            data: "[[1-install]] text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "[[1-install]] text",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn escaped_inline_ref_square_brackets() {
            let doc = Parser::default().parse("Here you can read about tigers.\\[[tigers]]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.\\[[tigers]]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.[[tigers]]",
                        },
                        source: Span {
                            data: "Here you can read about tigers.\\[[tigers]]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.\\[[tigers]]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }

        #[test]
        fn escaped_inline_ref_macro() {
            let doc = Parser::default().parse("Here you can read about tigers.\\anchor:tigers[]");

            assert_eq!(
                doc,
                Document {
                    header: Header {
                        title_source: None,
                        title: None,
                        attributes: &[],
                        author_line: None,
                        revision_line: None,
                        comments: &[],
                        source: Span {
                            data: "",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here you can read about tigers.\\anchor:tigers[]",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Here you can read about tigers.anchor:tigers[]",
                        },
                        source: Span {
                            data: "Here you can read about tigers.\\anchor:tigers[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "Here you can read about tigers.\\anchor:tigers[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog::default(),
                }
            );
        }
    }
}
