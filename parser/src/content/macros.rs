use std::{borrow::Cow, path::Path, sync::LazyLock};

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use regex::{Captures, Match, Regex, Replacer};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, content::XrefSegment},
    document::InterpretedValue,
    internal::{LookaheadReplacer, LookaheadResult, replace_with_lookahead},
    parser::{
        FootnoteRenderParams, IconRenderParams, ImageRenderParams, IndexTermRenderParams,
        LinkRenderParams, LinkRenderType, MenuRenderParams, XrefStyle,
    },
    warnings::WarningType,
};

pub(super) fn apply_macros(content: &mut Content<'_>, parser: &Parser) {
    let /* mut */ text = content.rendered().to_string();
    let found_square_bracket = text.contains('[');
    let found_colon = text.contains(':');
    let found_macroish = found_square_bracket && found_colon;
    let found_macroish_short = found_macroish && text.contains(":[");

    // A bibliography anchor (`[[[id]]]` / `[[[id,xreftext]]]`) is recognized only
    // when it prefixes the principal text of a bibliography list item; the parser
    // sets a flag while substituting that text. This runs before the inline-anchor
    // pass below so the prefix anchor is consumed as a whole rather than being
    // mistaken for a regular inline anchor (`[[id]]`) wrapped in square brackets.
    // The regex is `^`-anchored, so a `[[[…]]]` appearing later in the entry falls
    // through to the inline-anchor pass (matching Asciidoctor).
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

    // The UI macros (`kbd:`, `btn:`, and `menu:`) are recognized only when the
    // `experimental` document attribute is set. Although the UI macros are a
    // stable part of the AsciiDoc language, requiring the attribute is an
    // optimization that lets the processor skip this work in the common case.
    //
    // Adapted from Asciidoctor's #sub_macros, found in
    // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb#L349-L411.
    //
    // NOTE: The shorthand menu syntax (`"File > Save"`, handled by Asciidoctor's
    // `InlineMenuRx`) is intentionally not implemented; per the spec it is not
    // on a standards track.
    if parser.is_attribute_set("experimental") {
        if found_macroish_short && (text.contains("kbd:") || text.contains("btn:")) {
            let replacer = InlineKbdBtnMacroReplacer(parser);

            if let Cow::Owned(new_result) =
                INLINE_KBD_BTN_MACRO.replace_all(content.rendered(), replacer)
            {
                content.rendered = new_result.into();
            }
        }

        if found_macroish && text.contains("menu:") {
            let replacer = InlineMenuMacroReplacer(parser);

            if let Cow::Owned(new_result) =
                INLINE_MENU_MACRO.replace_all(content.rendered(), replacer)
            {
                content.rendered = new_result.into();
            }
        }
    }

    if found_macroish && (text.contains("image:") || text.contains("icon:")) {
        let replacer = InlineImageMacroReplacer(parser);

        if let Cow::Owned(new_result) = INLINE_IMAGE_MACRO.replace_all(content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    if (text.contains("((") && text.contains("))"))
        || (found_macroish_short && text.contains("dexterm"))
    {
        let replacer = InlineIndextermReplacer(parser);

        if let Cow::Owned(new_result) =
            replace_with_lookahead(&INLINE_INDEXTERM, content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

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
    //
    // This runs *before* footnotes so that a cross-reference inside a footnote
    // becomes a (bracket-free) placeholder before the footnote text is
    // extracted. That lets the footnote text — including the `xref:id[…]` macro
    // form, whose literal `]` would otherwise truncate the footnote — be
    // captured intact, and lets the footnote re-home the placeholder so it is
    // resolved in the document-level pass too.
    let mut xrefs: Vec<XrefSegment> = vec![];

    if (text.contains("&lt;&lt;") || (found_macroish && text.contains("xref:")))
        && let Cow::Owned(new_result) = INLINE_XREF.replace_all(
            content.rendered(),
            InlineXrefReplacer {
                parser,
                xrefs: &mut xrefs,
            },
        )
    {
        content.rendered = new_result.into();
    }

    // Footnotes (`footnote:[text]`, `footnote:id[text]`, `footnote:id[]`, and
    // the deprecated `footnoteref:[id,text]` / `footnoteref:[id]`).
    //
    // The footnote *text* is extracted out of the flow of text (only a
    // superscript marker is left behind), so any macro inside the footnote that
    // has already been substituted at this point (images, links, anchors, index
    // terms, and now cross-references) is captured as part of the footnote text.
    // Any cross-reference placeholders captured this way are re-homed onto the
    // footnote so they resolve in the document-level pass.
    if found_macroish && text.contains("tnote") {
        let replacer = InlineFootnoteMacroReplacer {
            parser,
            source: content.original(),
            all_xrefs: &xrefs,
        };

        if let Cow::Owned(new_result) =
            replace_with_lookahead(&INLINE_FOOTNOTE_MACRO, content.rendered(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    content.set_deferred_xrefs(xrefs);
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

/// Matches a keyboard (`kbd:[…]`) or button (`btn:[…]`) UI macro.
///
/// ## Examples
///
/// * `kbd:[F3]`
/// * `kbd:[Ctrl+Shift+T]`
/// * `kbd:[Ctrl+\]]`
/// * `btn:[Save]`
static INLINE_KBD_BTN_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                    # extended mode; dot matches newline
        (\\)?                       # (1) optional escape backslash
        (kbd|btn):                  # (2) macro name
        \[
            ( .*?[^\\] )            # (3) bracketed content, ending in a non-backslash
        \]
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineKbdBtnMacroReplacer<'p>(&'p Parser);

impl Replacer for InlineKbdBtnMacroReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // Honor the escape: emit the macro text without the leading backslash.
        if caps.get(1).is_some() {
            dest.push_str(&caps[0][1..]);
            return;
        }

        if &caps[2] == "kbd" {
            let keys = split_kbd_keys(&caps[3]);
            self.0.renderer.render_keyboard(&keys, dest);
        } else {
            // A button label is normalized like other bracketed macro text:
            // surrounding whitespace and newlines are folded, and any escaped
            // closing bracket is unescaped.
            let text = normalize_index_text(&caps[3], true);
            self.0.renderer.render_button(&text, dest);
        }
    }
}

/// Splits the raw argument of a `kbd:[…]` macro into individual keys, mirroring
/// Asciidoctor's delimiter handling.
///
/// A single key produces a one-element vector; a key sequence is split on the
/// first delimiter found — a comma (`,`) or a plus (`+`) — searching from the
/// *second* character so that a leading delimiter is treated as a literal key
/// (e.g. `kbd:[,te]` is the single key `,te`). If the argument ends with the
/// delimiter, that trailing delimiter is preserved as the value of the final
/// key (e.g. `kbd:[Ctrl + +]` yields `Ctrl` and `+`).
fn split_kbd_keys(raw: &str) -> Vec<String> {
    let mut keys = raw.trim().to_string();
    if keys.contains(']') {
        keys = keys.replace("\\]", "]");
    }

    // The delimiter is the earliest comma or plus that is not the first
    // character. Scanning from the second character and taking the first match
    // yields the same choice as Asciidoctor's `min` of the two candidate
    // indexes. Because the scan starts at the second character, a single-key
    // argument (or one whose only delimiter is a leading literal) yields `None`
    // here, so no separate length check is needed.
    let delim = keys.chars().skip(1).find(|c| *c == ',' || *c == '+');

    if let Some(delim) = delim {
        let ends_with_delim = keys.ends_with(delim);

        // Drop the trailing delimiter before splitting; it is restored on the
        // last key below. (Rust's `split` keeps trailing empty segments, which
        // matches Asciidoctor's `split delim, -1`.)
        let split_source = if ends_with_delim {
            &keys[..keys.len() - delim.len_utf8()]
        } else {
            keys.as_str()
        };

        let mut parts: Vec<String> = split_source
            .split(delim)
            .map(|k| k.trim().to_string())
            .collect();

        if ends_with_delim && let Some(last) = parts.last_mut() {
            last.push(delim);
        }

        parts
    } else {
        vec![keys]
    }
}

/// Matches a menu (`menu:…[…]`) UI macro.
///
/// The shorthand form (`"File > Save"`) is intentionally not matched here; per
/// the spec it is not on a standards track.
///
/// ## Examples
///
/// * `menu:File[]`
/// * `menu:File[Save]`
/// * `menu:View[Zoom > Reset]`
/// * `menu:Tools[Project, Build]`
static INLINE_MENU_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                        # extended mode; dot matches newline
        \\?                             # optional escape backslash (not captured)
        menu:
        (                               # (1) menu name
            \w                              # a single word character
          |                                 # or
            [\w&] [^\n\[]* [^\s\[]           # first word/ampersand char, then any run
                                            # not containing a newline or '[', ending
                                            # in a non-space, non-'[' character
        )
        \[ \x20*                        # opening '[' then optional spaces
        (?:                             # menu items (optional)
            |                               # empty
            ( .*?[^\\] )                    # (2) items, ending in a non-backslash
        )
        \]
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineMenuMacroReplacer<'p>(&'p Parser);

impl Replacer for InlineMenuMacroReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // Honor the escape: emit the macro text without the leading backslash.
        if caps[0].starts_with('\\') {
            dest.push_str(&caps[0][1..]);
            return;
        }

        let menu = &caps[1];

        // The items list, if present, is split into zero or more submenus and a
        // trailing menu item. The `&gt;` delimiter (already substituted from
        // `>`) takes precedence over a comma; without either, the whole list is
        // a single menu item.
        let (submenus, menuitem): (Vec<String>, Option<String>) = if let Some(items) = caps.get(2) {
            let mut items = items.as_str().to_string();
            if items.contains(']') {
                items = items.replace("\\]", "]");
            }

            let delim = if items.contains("&gt;") {
                Some("&gt;")
            } else if items.contains(',') {
                Some(",")
            } else {
                None
            };

            if let Some(delim) = delim {
                let mut parts: Vec<String> =
                    items.split(delim).map(|i| i.trim().to_string()).collect();
                let menuitem = parts.pop();
                (parts, menuitem)
            } else {
                (vec![], Some(items.trim_end().to_string()))
            }
        } else {
            (vec![], None)
        };

        let params = MenuRenderParams {
            menu,
            submenus: &submenus,
            menuitem: menuitem.as_deref(),
            parser: self.0,
        };

        self.0.renderer.render_menu(&params, dest);
    }
}

fn normalize_text_lf_escaped_bracket(text: &str) -> String {
    text.replace("\n", " ").replace("\\]", "]")
}

/// Matches an [index term] inline macro, in either the macro form
/// (`indexterm:[…]` / `indexterm2:[…]`) or the shorthand form
/// (`(((primary, secondary, tertiary)))` / `((primary))`).
///
/// The shorthand alternative captures the text between the outermost `((` and
/// `))`. Asciidoctor anchors the closing `))` with a `(?!\))` look-ahead so
/// that the *last* pair in a run of parentheses closes the term; Rust's regex
/// engine has no look-ahead, so [`InlineIndextermReplacer`] re-creates that
/// behavior by absorbing any trailing `)` that follow the matched `))`.
///
/// [index term]: https://docs.asciidoctor.org/asciidoc/latest/sections/user-index/
static INLINE_INDEXTERM: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                         # extended mode; dot matches newline
        \\?                              # optional escaping backslash
        (?:
            (indexterm2?):\[ (.*?[^\\]) \]   # (1) macro name, (2) macro argument
          |
            \(\( (.+?) \)\)                  # (3) shorthand enclosed text
        )
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineIndextermReplacer<'p>(&'p Parser);

impl LookaheadReplacer for InlineIndextermReplacer<'_> {
    fn replace_append(
        &mut self,
        caps: &Captures<'_>,
        dest: &mut String,
        after: &str,
    ) -> LookaheadResult {
        // Adapted from Asciidoctor#sub_macros (the `InlineIndextermMacroRx`
        // branch), found in
        // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb.

        let parser = self.0;

        // Macro form: `indexterm:[…]` (concealed) or `indexterm2:[…]` (flow).
        if let Some(name) = caps.get(1) {
            // Honor the escape: emit the macro text without the backslash.
            if caps[0].starts_with('\\') {
                dest.push_str(&caps[0][1..]);
                return LookaheadResult::Continue;
            }

            if name.as_str() == "indexterm2" {
                // A flow index term renders its primary term inline. When the
                // argument carries an attribute list (it contains `=`), the
                // first positional attribute is the primary term.
                let arg = normalize_index_text(&caps[2], true);
                let term = if arg.contains('=') {
                    Attrlist::parse(Span::new(&arg), parser, AttrlistContext::Inline)
                        .item
                        .item
                        .nth_attribute(1)
                        .map(|a| a.value().to_string())
                        .unwrap_or(arg)
                } else {
                    arg
                };

                parser.renderer.render_index_term(
                    &IndexTermRenderParams {
                        visible_term: Some(&term),
                    },
                    dest,
                );
            } else {
                // A concealed index term produces no inline output.
                parser
                    .renderer
                    .render_index_term(&IndexTermRenderParams { visible_term: None }, dest);
            }

            return LookaheadResult::Continue;
        }

        // Shorthand form: `((…))` / `(((…)))`.
        //
        // Absorb any `)` that immediately follow the matched `))` so that the
        // closing pair is the last in the run, mirroring Asciidoctor's
        // `(?!\))` look-ahead. Those extra characters are part of this logical
        // match, so they are skipped (rather than re-scanned) once consumed.
        let extra = after.bytes().take_while(|b| *b == b')').count();
        let advance = if extra > 0 {
            LookaheadResult::SkipAheadAndRetry(caps[0].len() + extra)
        } else {
            LookaheadResult::Continue
        };

        let mut encl_text = String::with_capacity(caps[3].len() + extra);
        encl_text.push_str(&caps[3]);
        for _ in 0..extra {
            encl_text.push(')');
        }

        let escaped = caps[0].starts_with('\\');

        // Strip the enclosing parentheses (if any) to decide whether the term
        // is concealed or visible, and which literal parentheses to preserve in
        // the flow of text. `before`/`trailing` carry literal parentheses that
        // are adjacent to (but not part of) the index term.
        let (inner, visible, before, trailing): (&str, bool, &str, &str) = if escaped {
            if encl_text.starts_with('(') && encl_text.ends_with(')') {
                // An escaped concealed term still processes a nested flow term.
                (&encl_text[1..encl_text.len() - 1], true, "(", ")")
            } else {
                // Honor the escape: emit the enclosed text verbatim (the full
                // match, including any absorbed parens, minus the backslash).
                dest.push_str(&caps[0][1..]);
                for _ in 0..extra {
                    dest.push(')');
                }
                return advance;
            }
        } else if let Some(without_open) = encl_text.strip_prefix('(') {
            if let Some(inner) = without_open.strip_suffix(')') {
                // `(((concealed)))`
                (inner, false, "", "")
            } else {
                (without_open, true, "(", "")
            }
        } else if let Some(inner) = encl_text.strip_suffix(')') {
            (inner, true, "", ")")
        } else {
            // `((visible))`
            (&encl_text[..], true, "", "")
        };

        dest.push_str(before);

        if visible {
            let term = strip_see_and_seealso(&normalize_index_text(inner, false));
            parser.renderer.render_index_term(
                &IndexTermRenderParams {
                    visible_term: Some(&term),
                },
                dest,
            );
        } else {
            parser
                .renderer
                .render_index_term(&IndexTermRenderParams { visible_term: None }, dest);
        }

        dest.push_str(trailing);

        advance
    }
}

/// Normalizes the text of an index term: trims surrounding whitespace and
/// collapses embedded newlines to spaces (Asciidoctor compacts a multi-line
/// term onto a single line). When `unescape_brackets` is set (the macro forms),
/// an escaped closing square bracket (`\]`) is also unescaped.
fn normalize_index_text(text: &str, unescape_brackets: bool) -> String {
    let normalized = text.trim().replace('\n', " ");
    if unescape_brackets {
        normalized.replace("\\]", "]")
    } else {
        normalized
    }
}

/// Strips a trailing `see` (` >> …`) or `see-also` (` &> …`) clause from a
/// visible index term, leaving only the primary term to display in the flow of
/// text. By the time macros are processed, the special-characters substitution
/// has already turned `>` into `&gt;` and `&` into `&amp;`, so the separators
/// appear here as ` &gt;&gt; ` and ` &amp;&gt; `.
fn strip_see_and_seealso(term: &str) -> String {
    // Cheap guard mirroring Asciidoctor's `term.include? ';&'`.
    if term.contains(";&") {
        if let Some((primary, _see)) = term.split_once(" &gt;&gt; ") {
            return primary.to_string();
        }
        if let Some((primary, _see_also)) = term.split_once(" &amp;&gt; ") {
            return primary.to_string();
        }
    }
    term.to_string()
}

// Ruby Asciidoctor's `InlineLinkRx` gates the angle-bracketed-URL alternative
// (`\2([^\s]+?)&gt;`) with a back-reference to the `&lt;`-prefix capture group,
// so it fires *only* when a leading `&lt;` was seen. The `regex` crate has no
// back-references, so we can't express that gate inline. Instead we split the
// pattern into two parallel top-level branches:
//
//   * the ANGLE branch requires a literal `&lt;` prefix and therefore keeps the
//     angle-bracketed-URL alternative, and
//   * the NON-ANGLE branch omits that alternative entirely.
//
// A stray `&gt;` with no matching `&lt;` (e.g. `https://example.org>;`) can then
// only match the bare-link alternative, exactly as Ruby routes it. See #503.
//
// `InlineLinkReplacer` normalizes the two capture-group sets into a single view
// (see `NormalizedCaps`), so the numbering below is only referenced there.
static INLINE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?msx)
        (?:
            #### ANGLE branch: prefix is `&lt;`, keeping the `&gt;` alternative.
            ( \\?&lt; )                                       # group 1: prefix
            ( \\? (?: https? | file | ftp | irc ):// )        # group 2: scheme
            (?:
                ( [^\s\[\]]+ )                                # group 3: target
                \[ ( | .*?[^\\] ) \]                          # group 4: attrlist
              | ( [^\s]+? ) &gt;                              # group 5: URL inside <>
              | ( [^\s\[\]<]* ( [^\s,.?!\[\]<\)] ) )          # group 6: bare link,
                                                              # group 7: trailing char
            )
          |
            #### NON-ANGLE branch: no `&gt;` alternative (unreachable without `&lt;`).
            ( ^ | link: | [\ \t] | [>\(\)\[\];"'] )           # group 8: prefix
            ( \\? (?: https? | file | ftp | irc ):// )        # group 9: scheme
            (?:
                ( [^\s\[\]]+ )                                # group 10: target
                \[ ( | .*?[^\\] ) \]                          # group 11: attrlist
              | ( [^\s\[\]<]* ( [^\s,.?!\[\]<\)] ) )          # group 12: bare link,
                                                              # group 13: trailing char
            )
        )
    "#,
    )
    .unwrap()
});

/// A branch-agnostic view over the capture groups of [`INLINE_LINK`], which has
/// two parallel top-level branches (angle / non-angle). Exactly one branch
/// participates in any given match; this resolves the relevant groups so the
/// replacer doesn't have to special-case the branch numbering everywhere.
struct NormalizedCaps<'c, 't> {
    caps: &'c Captures<'t>,
    /// True when the ANGLE branch matched (prefix was `&lt;`). Corresponds to
    /// the `&lt;` flag (old capture group 2) in the Ruby implementation.
    is_angle: bool,
    prefix: usize,
    scheme: usize,
    /// Formal-macro target: the URL preceding a `[…]` attrlist.
    target: usize,
    attrlist: usize,
    /// URL captured inside `<…&gt;`; only present in the ANGLE branch.
    angle_url: Option<usize>,
    /// Bare (auto-linked) URL.
    bare: usize,
}

impl<'c, 't> NormalizedCaps<'c, 't> {
    fn new(caps: &'c Captures<'t>) -> Self {
        if caps.get(1).is_some() {
            NormalizedCaps {
                caps,
                is_angle: true,
                prefix: 1,
                scheme: 2,
                target: 3,
                attrlist: 4,
                angle_url: Some(5),
                bare: 6,
            }
        } else {
            NormalizedCaps {
                caps,
                is_angle: false,
                prefix: 8,
                scheme: 9,
                target: 10,
                attrlist: 11,
                angle_url: None,
                bare: 12,
            }
        }
    }

    fn prefix(&self) -> &'t str {
        self.caps.get(self.prefix).map_or("", |m| m.as_str())
    }

    fn scheme(&self) -> &'t str {
        self.caps.get(self.scheme).map_or("", |m| m.as_str())
    }

    fn target(&self) -> Option<Match<'t>> {
        self.caps.get(self.target)
    }

    fn attrlist(&self) -> Option<Match<'t>> {
        self.caps.get(self.attrlist)
    }

    fn angle_url(&self) -> Option<Match<'t>> {
        self.angle_url.and_then(|g| self.caps.get(g))
    }

    fn bare(&self) -> Option<Match<'t>> {
        self.caps.get(self.bare)
    }
}

#[derive(Debug)]
struct InlineLinkReplacer<'p>(&'p Parser);

impl Replacer for InlineLinkReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        let mut attrlist = Attrlist::parse(Span::default(), self.0, AttrlistContext::Inline)
            .item
            .item;

        // `INLINE_LINK` has two parallel top-level branches (angle / non-angle);
        // resolve which one matched so the logic below can stay branch-agnostic.
        // See the note on `INLINE_LINK` and issue #503.
        let n = NormalizedCaps::new(caps);
        let prefix_match = n.prefix();
        let scheme_match = n.scheme();

        if n.is_angle && n.attrlist().is_none() {
            // Honor the escapes.
            if prefix_match.starts_with('\\') {
                dest.push_str(&caps[0][1..]);
                return;
            }

            if scheme_match.starts_with('\\') {
                dest.push_str(prefix_match);
                dest.push_str(&caps[0][prefix_match.len() + 1..]);
                return;
            }

            let Some(link_suffix) = n.angle_url() else {
                dest.push_str(&caps[0]);
                return;
            };

            let target = format!(
                "{scheme}{link_suffix}",
                scheme = scheme_match,
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

        let mut prefix = prefix_match.to_string();
        let scheme = scheme_match;

        // Honor the escape.
        if scheme.starts_with('\\') {
            dest.push_str(&prefix);
            dest.push_str(&caps[0][prefix.len() + 1..]);
            return;
        }

        // The target and bare-link groups are mutually exclusive regex
        // alternatives; exactly one is `Some(_)` when we reach this point.
        // `target` = formal macro target (URL before '['); `bare` = bare link.
        //
        // The angle-bracketed-URL alternative (`angle_url`) only exists in the
        // ANGLE branch, and that case returns above (before an attrlist can be
        // present), so it never contributes here. A stray `&gt;` with no leading
        // `&lt;` therefore lands in the bare-link group and keeps its literal
        // `&gt;`, with any trailing punctuation stripped by the rule below --
        // matching Ruby Asciidoctor (see issue #503).
        let url_part = n
            .target()
            .or_else(|| n.bare())
            .map(|m| m.as_str().to_owned())
            .unwrap_or_default();
        let mut target = format!("{scheme}{url_part}");

        let mut suffix = "".to_owned();
        let mut link_text: Option<String> = None;

        // NOTE: If the attrlist group matched, we're looking at a formal macro (e.g., https://example.org[]).
        if let Some(attrlist) = n.attrlist() {
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
            // URL. Keying off the target's final character rather than the
            // trailing-char capture group handles a bare link that ends in a
            // literal `&gt;` (whose final character is ';'), matching Ruby.
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

/// Matches a bibliography anchor that prefixes a bibliography list item.
///
/// The anchor is matched only at the very start of the entry (`^`), mirroring
/// Asciidoctor: a `[[[…]]]` appearing later in the text is left to the regular
/// inline-anchor pass. The label must be _non-numeric_ (it may contain digits,
/// but must not begin with one), so an entry that opens with something like
/// `[[[1984]]]` is left untouched. An optional xreftext follows a comma.
///
/// A leading backslash is deliberately *not* accepted as an escape: `\[[[id]]]`
/// does not begin with `[[[`, so it simply isn't a bibliography anchor (the
/// backslash and inner `[[id]]` are handled by the inline-anchor pass, matching
/// Asciidoctor). The documented escape `[\[[id]]]` likewise does not start with
/// `[[[` and is handled there.
///
/// ## Examples
///
/// * `[[[label]]]`
/// * `[[[label,xreftext]]]`
static INLINE_BIBLIO_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
        ^                               # the anchor must prefix the entry
        \[\[\[                          # opening triple bracket
          (                             # (1) bibliography label
            [\p{Alphabetic}_:]              # first char: letter, '_' or ':' (never a digit)
            [\p{Alphabetic}\p{Nd}_\-:.]*    # rest: letters/digits/_/-/:/.
          )
          (?: , \s* (.+?) )?            # (2) optional xreftext after a comma
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
        let id = &caps[1];

        // The displayed reference text is the xreftext if supplied, otherwise the
        // label itself, always enclosed in square brackets (e.g. `[gof]`). This
        // same bracketed text is registered as the entry's reftext so a
        // cross-reference to the entry renders identically.
        let label = caps.get(2).map(|m| m.as_str()).unwrap_or(id);
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
struct InlineXrefReplacer<'p, 'x> {
    parser: &'p Parser,

    /// Accumulates the cross-references discovered during replacement, in the
    /// same order as the placeholders emitted into the output.
    xrefs: &'x mut Vec<XrefSegment>,
}

/// Reads the document-wide `xrefstyle` attribute as an [`XrefStyle`].
///
/// An unset attribute yields `None` (the target's reftext is used verbatim). A
/// set-but-empty value (`:xrefstyle:`) and any unrecognized value both resolve
/// to [`XrefStyle::Basic`], mirroring Asciidoctor.
fn document_xrefstyle(parser: &Parser) -> Option<XrefStyle> {
    match parser.attribute_value("xrefstyle") {
        InterpretedValue::Value(value) => Some(XrefStyle::parse(&value)),
        InterpretedValue::Set => Some(XrefStyle::Basic),
        InterpretedValue::Unset => None,
    }
}

impl Replacer for InlineXrefReplacer<'_, '_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps.get(1).is_some() {
            // Honor the escape: emit the reference literally (sans backslash).
            dest.push_str(&caps[0][1..]);
            return;
        }

        let mut window: Option<String> = None;
        let mut roles: Vec<String> = vec![];

        // A `xrefstyle=` attribute on the `xref:` macro overrides the
        // document-wide `xrefstyle` for this one reference.
        let mut xrefstyle_override: Option<XrefStyle> = None;

        let (target, provided_text) = if let Some(inner) = caps.get(2) {
            // Shorthand form: split an optional ", reftext" off the id. The id
            // is always treated as a same-document reference, even when it
            // contains a dot.
            match inner.as_str().split_once(',') {
                Some((id, text)) => (id.trim().to_string(), Some(text.trim().to_string())),
                None => (inner.as_str().trim().to_string(), None),
            }
        } else {
            // `xref:` macro form. A target that begins with `#` is an explicit
            // same-document reference (the hash is dropped); any other target
            // that contains a dot is treated as an inter-document reference and
            // left for a host-supplied resolver to interpret.
            let raw_target = &caps[3];
            let target = raw_target
                .strip_prefix('#')
                .unwrap_or(raw_target)
                .to_string();

            // The bracketed text is parsed as an attribute list when it contains
            // an `=` (mirroring the link macro): the first positional attribute
            // is the link text, and named attributes such as `window` and `role`
            // are honored. Otherwise the whole text is the link text.
            let raw_text = caps.get(4).map(|m| m.as_str()).unwrap_or_default();

            let provided_text = if raw_text.is_empty() {
                None
            } else if raw_text.contains('=') {
                let normalized = raw_text.replace('\n', " ");
                let attrlist =
                    Attrlist::parse(Span::new(&normalized), self.parser, AttrlistContext::Inline)
                        .item
                        .item;

                window = attrlist
                    .named_attribute("window")
                    .map(|a| a.value().to_string());
                roles = attrlist.roles().iter().map(|r| r.to_string()).collect();
                xrefstyle_override = attrlist
                    .named_attribute("xrefstyle")
                    .map(|a| XrefStyle::parse(a.value()));

                attrlist
                    .nth_attribute(1)
                    .map(|a| a.value().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                Some(raw_text.replace("\\]", "]"))
            };

            (target, provided_text)
        };

        // The effective style is the macro-level override if present, otherwise
        // the document-wide `xrefstyle` at this point in the document.
        let xrefstyle = xrefstyle_override.or_else(|| document_xrefstyle(self.parser));

        let index = self.xrefs.len();
        self.xrefs.push(XrefSegment {
            target,
            provided_text,
            window,
            roles,
            xrefstyle,
            resolved: None,
        });

        dest.push_str(&Content::xref_placeholder(index));
    }
}

/// Matches a [footnote] inline macro, in either the `footnote:` form or the
/// deprecated `footnoteref:` form.
///
/// ## Examples
///
/// * `footnote:[text]` — an anonymous footnote
/// * `footnote:id[text]` — a footnote with an ID, so it can be referenced again
/// * `footnote:id[]` — a reference to a previously-defined footnote
/// * `footnoteref:[id,text]` / `footnoteref:[id]` — the deprecated equivalents
///
/// Asciidoctor anchors the match with a `(?!</a>)` look-ahead after the closing
/// bracket so a `footnote:[…]` that forms the text of an already-rendered link
/// is not matched again; the `regex` crate has no look-ahead, so
/// [`InlineFootnoteMacroReplacer`] re-creates that guard by inspecting the text
/// that follows the match.
///
/// [footnote]: https://docs.asciidoctor.org/asciidoc/latest/macros/footnote/
static INLINE_FOOTNOTE_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                     # extended mode; dot matches newline
        \\?                          # optional escaping backslash
        footnote
        (?:
            (ref):                   # (1) the deprecated 'footnoteref:' form
          |
            : ([\w-]+)?              # (2) optional id for the 'footnote:id' form
        )
        \[
            (?: | (.*?[^\\]) )       # (3) text: empty, or ends in a non-backslash
        \]
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlineFootnoteMacroReplacer<'p, 's, 'x> {
    parser: &'p Parser,

    /// The original (pre-substitution) span of the content being rendered, used
    /// to locate any warning recorded while resolving a footnote.
    source: Span<'s>,

    /// The enclosing block's cross-references, produced by the (earlier)
    /// cross-reference pass. A footnote whose text contains a cross-reference
    /// placeholder re-homes the referenced segments out of this list onto
    /// itself.
    all_xrefs: &'x [XrefSegment],
}

impl LookaheadReplacer for InlineFootnoteMacroReplacer<'_, '_, '_> {
    fn replace_append(
        &mut self,
        caps: &Captures<'_>,
        dest: &mut String,
        after: &str,
    ) -> LookaheadResult {
        // Adapted from Asciidoctor#sub_macros (the `InlineFootnoteMacroRx`
        // branch), found in
        // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb.

        // Honor the escape: emit the macro text without the leading backslash.
        if caps[0].starts_with('\\') {
            dest.push_str(&caps[0][1..]);
            return LookaheadResult::Continue;
        }

        // Re-create Asciidoctor's `(?!</a>)` look-ahead: a closing bracket
        // immediately followed by `</a>` is not a footnote (it closes an
        // already-rendered link), so the macro is left untouched.
        if after.starts_with("</a>") {
            dest.push_str(&caps[0]);
            return LookaheadResult::Continue;
        }

        let parser = self.parser;

        // Resolve the macro into an (id, text) pair. The deprecated
        // `footnoteref:` form packs both into the bracketed text (`id,text`),
        // whereas the `footnote:` form takes the id from the macro target.
        let (id, content): (Option<String>, Option<String>) = if caps.get(1).is_some() {
            // `footnoteref:` form. With no bracketed text at all it is left
            // untouched (matching Asciidoctor's `next $&`).
            let Some(raw) = caps.get(3).map(|m| m.as_str()) else {
                dest.push_str(&caps[0]);
                return LookaheadResult::Continue;
            };

            // The `footnoteref:` macro is deprecated outside compatibility mode.
            if !parser.is_attribute_set("compat-mode") {
                parser.record_substitution_warning(
                    self.source,
                    WarningType::DeprecatedFootnorefMacro(caps[0].to_string()),
                );
            }

            match raw.split_once(',') {
                Some((id, content)) => (Some(id.to_string()), Some(content.to_string())),
                None => (Some(raw.to_string()), None),
            }
        } else {
            // `footnote:` form.
            (
                caps.get(2).map(|m| m.as_str().to_string()),
                caps.get(3).map(|m| m.as_str().to_string()),
            )
        };

        // While a section title is substituted, bracket a real footnote's marker
        // with sentinels so it can later be excised from the section's reference
        // text and auto-generated ID (see `Parser::mark_footnote_spans`). The
        // footnote is still defined and numbered here, in document order; only
        // the marker's *placement* is annotated. A bare `footnote:[]` (the
        // literal-text branch below) is not a footnote and is left unmarked.
        let mark_span = parser.mark_footnote_spans.get() && (id.is_some() || content.is_some());
        if mark_span {
            dest.push(crate::content::FOOTNOTE_MARKER_START);
        }

        // `id` and `content` own their data, so each branch renders its marker
        // before they are dropped (the params borrow them).
        if let Some(id) = id {
            if let Some(index) = parser.footnote_index_for_id(&id) {
                // A reference to an already-defined footnote: reuse its number.
                parser.renderer.render_footnote(
                    &FootnoteRenderParams {
                        index: Some(index.as_str()),
                        id: None,
                        is_reference: true,
                        text: "",
                    },
                    dest,
                );
            } else if let Some(content) = content {
                // A defining occurrence that also carries an ID.
                let (template, xrefs) = crate::content::rehome_xref_placeholders(
                    &normalize_footnote_text(&content),
                    self.all_xrefs,
                );
                let index = parser.define_footnote(Some(&id), template, xrefs);
                parser.renderer.render_footnote(
                    &FootnoteRenderParams {
                        index: Some(index.as_str()),
                        id: Some(&id),
                        is_reference: false,
                        text: "",
                    },
                    dest,
                );
            } else {
                // A reference to an ID that was never defined.
                parser.record_substitution_warning(
                    self.source,
                    WarningType::InvalidFootnoteReference(id.clone()),
                );
                parser.renderer.render_footnote(
                    &FootnoteRenderParams {
                        index: None,
                        id: None,
                        is_reference: true,
                        text: &id,
                    },
                    dest,
                );
            }
        } else if let Some(content) = content {
            // An anonymous defining occurrence.
            let (template, xrefs) = crate::content::rehome_xref_placeholders(
                &normalize_footnote_text(&content),
                self.all_xrefs,
            );
            let index = parser.define_footnote(None, template, xrefs);
            parser.renderer.render_footnote(
                &FootnoteRenderParams {
                    index: Some(index.as_str()),
                    id: None,
                    is_reference: false,
                    text: "",
                },
                dest,
            );
        } else {
            // `footnote:[]` with neither an ID nor text is not a footnote.
            dest.push_str(&caps[0]);
        }

        if mark_span {
            dest.push(crate::content::FOOTNOTE_MARKER_END);
        }

        LookaheadResult::Continue
    }
}

/// Normalizes the text of a footnote: trims surrounding whitespace, collapses
/// each embedded newline to a space (Asciidoctor compacts a multi-line footnote
/// onto a single line), and unescapes an escaped closing square bracket
/// (`\]` -> `]`). Mirrors Asciidoctor's `normalize_text text, true, true`.
fn normalize_footnote_text(content: &str) -> String {
    content.trim().replace('\n', " ").replace("\\]", "]")
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

        #[test]
        fn stray_gt_followed_by_punctuation() {
            // Regression for https://github.com/asciidoc-rs/asciidoc-parser/issues/503:
            // a bare URL abutting `>;` (rendered to `&gt;;`) with NO matching
            // leading `<`. Ruby Asciidoctor treats the whole run as a bare link
            // (keeping the literal `&gt;` in the URL) and strips a single trailing
            // `;`.
            //
            // Reference (Ruby Asciidoctor 2.0.23):
            //   $ printf '%s' 'foo https://example.org>;' | asciidoctor -e -o - -
            //   <p>foo <a href="https://example.org&gt;" class="bare">https://example.org&gt;</a>;</p>
            //
            // Previously the ungated angle-URL alternative fired for the stray
            // `&gt;` and split the run, dropping the `;` from the `&gt;` entity.
            let doc = Parser::default().parse("foo https://example.org>;");

            let rendered = doc
                .nested_blocks()
                .next()
                .unwrap()
                .rendered_content()
                .unwrap();

            assert_eq!(
                rendered,
                r#"foo <a href="https://example.org&gt;" class="bare">https://example.org&gt;</a>;"#
            );
        }

        #[test]
        fn angle_bracketed_url_still_matches() {
            // Companion to `stray_gt_followed_by_punctuation` (issue #503): the
            // genuine angle-bracketed autolink (`<url>`) must keep working. The
            // `&lt;` prefix gates the angle-URL alternative back on, so the `&gt;`
            // delimiter is consumed and the brackets are dropped from the link.
            let doc = Parser::default().parse("See <https://example.org> for details.");

            let rendered = doc
                .nested_blocks()
                .next()
                .unwrap()
                .rendered_content()
                .unwrap();

            assert_eq!(
                rendered,
                r#"See <a href="https://example.org" class="bare">https://example.org</a> for details."#
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

    mod bibliography_anchor {
        #![allow(clippy::indexing_slicing)]

        use crate::tests::prelude::*;

        #[test]
        fn recognized_only_when_it_prefixes_the_entry() {
            // A `[[[id]]]` that does not prefix the entry is not a bibliography
            // anchor: it falls through to the regular inline-anchor pass (matching
            // Asciidoctor), rendering as `[<a id="mid"></a>]` rather than the
            // bibliography form `<a id="mid"></a>[mid]`.
            let doc = Parser::default().parse("[bibliography]\n* Smith. See [[[mid]]] inline.\n");

            let rendered = &rendered_paragraphs(&doc)[0];
            assert!(
                rendered.contains("[<a id=\"mid\"></a>]"),
                "unexpected: {rendered}"
            );
            assert!(!rendered.contains("<a id=\"mid\"></a>[mid]"));

            // The entry is registered as a normal anchor, not a bibliography one.
            assert_eq!(
                doc.catalog().get_ref("mid").map(|e| e.ref_type.clone()),
                Some(crate::document::RefType::Anchor)
            );
        }

        #[test]
        fn leading_backslash_is_not_a_bibliography_escape() {
            // A leading backslash does not escape a bibliography anchor (the only
            // documented escape is `[\[[id]]]`). `\[[[id]]]` does not begin with
            // `[[[`, so it is not a bibliography anchor; the backslash stays
            // literal and the inner `[[id]]` becomes a normal inline anchor,
            // matching Asciidoctor's `\[<a id="x"></a>]`.
            let doc = Parser::default().parse("[bibliography]\n* \\[[[x]]] Leading backslash.\n");

            let rendered = &rendered_paragraphs(&doc)[0];
            assert!(
                rendered.starts_with("\\[<a id=\"x\"></a>]"),
                "unexpected: {rendered}"
            );
        }

        #[test]
        fn explicit_style_applies_to_an_ordered_list() {
            // An explicit `[bibliography]` attribute applies to any list type, so
            // an ordered list's entries are recognized as bibliography anchors,
            // matching Asciidoctor (`<div class="olist bibliography">`).
            let doc = Parser::default().parse("[bibliography]\n. [[[ord]]] Ordered entry.\n");

            assert_css(&doc, ".olist.bibliography", 1);
            assert!(rendered_paragraphs(&doc)[0].starts_with("<a id=\"ord\"></a>[ord] "));
        }

        #[test]
        fn section_style_does_not_apply_to_an_ordered_list() {
            // The style inherited from a `bibliography` section applies only to
            // unordered lists, so an ordered list in that section is not a
            // bibliography list; its leading `[[[id]]]` is a regular inline anchor.
            let doc = Parser::default()
                .parse("[bibliography]\n== References\n\n. [[[ord]]] Ordered entry.\n");

            assert_css(&doc, ".bibliography", 0);
            assert!(rendered_paragraphs(&doc)[0].starts_with("[<a id=\"ord\"></a>] "));
        }
    }
}
