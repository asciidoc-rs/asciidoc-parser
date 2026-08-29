use std::{path::Path, sync::LazyLock};

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use regex::{Captures, Match, Regex};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    document::InterpretedValue,
    parser::XrefStyle,
};

/// Matches an [inline image] (`image:target[…]`) or [inline icon]
/// (`icon:target[…]`) macro.
///
/// ## Examples
///
/// * `image:sunset.jpg[]`
/// * `image:sunset.jpg[Sunset,300,200]`
/// * `icon:tags[]`
/// * `icon:t[]` — a single-character target
///
/// The target is required; only its trailing portion is optional, so a
/// one-character target matches but an empty one does not. A macro written
/// without a target (`image:[]`) is thus left as literal text, matching
/// Asciidoctor's `InlineImageMacroRx`.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes image and icon
/// macros with the *exact* same pattern this string step matches with, changing
/// only the recognition *sink* (a node instead of rendered markup) — which is
/// what carries the target rule above onto the tree side unchanged.
///
/// [inline image]: https://docs.asciidoctor.org/asciidoc/latest/macros/images/
/// [inline icon]: https://docs.asciidoctor.org/asciidoc/latest/macros/icons/
pub(crate) static INLINE_IMAGE_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)                    
            \\?                         # Optional escape: literal backslash
            i(?:mage|con):              # 'image:' or 'icon:' prefix

            (                           # Group 1: the target (required)
                [^:\s\[\n]                  # First char: not colon, whitespace, [, or newline
                (?:                         # Remainder is optional: a target
                                            # may be a single character
                    [^\[\n]*?                   # Middle chars: any except [ or newline, lazily
                    [^\s\[\n]                   # Last char: not whitespace, [, or newline
                )?
            )

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

pub(crate) fn basename(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Matches a keyboard (`kbd:[…]`) or button (`btn:[…]`) UI macro.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes keyboard and
/// button macros with the *exact* same pattern this string step matches with,
/// changing only the recognition *sink* (a node instead of rendered markup).
///
/// ## Examples
///
/// * `kbd:[F3]`
/// * `kbd:[Ctrl+Shift+T]`
/// * `kbd:[Ctrl+\]]`
/// * `btn:[Save]`
pub(crate) static INLINE_KBD_BTN_MACRO: LazyLock<Regex> = LazyLock::new(|| {
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

/// Splits the raw argument of a `kbd:[…]` macro into individual keys, mirroring
/// Asciidoctor's delimiter handling.
///
/// A single key produces a one-element vector; a key sequence is split on the
/// first delimiter found — a comma (`,`) or a plus (`+`) — searching from the
/// *second* character so that a leading delimiter is treated as a literal key
/// (e.g. `kbd:[,te]` is the single key `,te`). If the argument ends with the
/// delimiter, that trailing delimiter is preserved as the value of the final
/// key (e.g. `kbd:[Ctrl + +]` yields `Ctrl` and `+`).
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) splits a keyboard macro's
/// keys exactly as this string step does.
pub(crate) fn split_kbd_keys(raw: &str) -> Vec<String> {
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
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes menu macros
/// with the *exact* same pattern this string step matches with, changing only
/// the recognition *sink* (a node instead of rendered markup).
///
/// ## Examples
///
/// * `menu:File[]`
/// * `menu:File[Save]`
/// * `menu:View[Zoom > Reset]`
/// * `menu:Tools[Project, Build]`
pub(crate) static INLINE_MENU_MACRO: LazyLock<Regex> = LazyLock::new(|| {
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

pub(crate) fn normalize_text_lf_escaped_bracket(text: &str) -> String {
    text.replace("\n", " ").replace("\\]", "]")
}

/// Matches an [index term] inline macro, in either the macro form
/// (`indexterm:[…]` / `indexterm2:[…]`) or the shorthand form
/// (`(((primary, secondary, tertiary)))` / `((primary))`).
///
/// The shorthand alternative captures the text between the outermost `((` and
/// `))`. Asciidoctor anchors the closing `))` with a `(?!\))` look-ahead so
/// that the *last* pair in a run of parentheses closes the term; Rust's regex
/// engine has no look-ahead, so `InlineIndextermReplacer` re-created that
/// behavior by absorbing any trailing `)` that follow the matched `))` —
/// as the tree builder's index-term family still does.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes index terms
/// with the *exact* same pattern this string step matches with, changing only
/// the recognition *sink* (a node instead of rendered markup).
///
/// [index term]: https://docs.asciidoctor.org/asciidoc/latest/sections/user-index/
pub(crate) static INLINE_INDEXTERM: LazyLock<Regex> = LazyLock::new(|| {
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

/// Normalizes the text of an index term: trims surrounding whitespace and
/// collapses embedded newlines to spaces (Asciidoctor compacts a multi-line
/// term onto a single line). When `unescape_brackets` is set (the macro forms),
/// an escaped closing square bracket (`\]`) is also unescaped.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) normalizes a button
/// macro's label exactly as this string step does.
pub(crate) fn normalize_index_text(text: &str, unescape_brackets: bool) -> String {
    let normalized = text.trim().replace('\n', " ");
    if unescape_brackets {
        normalized.replace("\\]", "]")
    } else {
        normalized
    }
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
// The blank alternative in the prefix (`[\ \t\p{Zs}]`) mirrors Asciidoctor's
// `CG_BLANK` (`\p{Blank}`), which under Ruby's Unicode-aware engine treats any
// space separator — including a no-break space (U+00A0) — as a boundary before
// the scheme. A plain ASCII `[\ \t]` would leave such a URL as literal text
// (see #768).
//
// The `link:` prefix is broken out into its own branch (below) that *requires*
// a trailing `[…]`, so a `link:` with no brackets simply fails to match here
// rather than matching as a bare link and then being rejected as invalid macro
// syntax in the replacer.
//
// `InlineLinkReplacer` normalizes the three capture-group sets into a single
// view (see `NormalizedCaps`), so the numbering below is only referenced there.
//
// Shared `pub(crate)` so the single-pass
// [`inline_builder`](crate::content::inline_builder) recognizes auto-links and
// formal-URL links with the *exact* same pattern this string step matches with,
// changing only the recognition *sink* (a node instead of rendered markup).
pub(crate) static INLINE_LINK: LazyLock<Regex> = LazyLock::new(|| {
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
            #### LINK-MACRO branch: a `link:` prefix, which REQUIRES a trailing
            #### `[…]`. Because this branch has no bare-link alternative, a
            #### `link:` followed by a URL but no brackets does not match at all
            #### (it is left as literal text), so it can never reach the
            #### invalid-macro-syntax path in the replacer.
            ( link: )                                         # group 8: prefix
            ( \\? (?: https? | file | ftp | irc ):// )        # group 9: scheme
            ( [^\s\[\]]+ )                                    # group 10: target
            \[ ( | .*?[^\\] ) \]                              # group 11: attrlist
          |
            #### NON-ANGLE branch: no `&gt;` alternative (unreachable without `&lt;`).
            ( ^ | [\ \t\p{Zs}] | [>\(\)\[\];"'] )             # group 12: prefix
            ( \\? (?: https? | file | ftp | irc ):// )        # group 13: scheme
            (?:
                ( [^\s\[\]]+ )                                # group 14: target
                \[ ( | .*?[^\\] ) \]                          # group 15: attrlist
              | ( [^\s\[\]<]* ( [^\s,.?!\[\]<\)] ) )          # group 16: bare link,
                                                              # group 17: trailing char
            )
        )
    "#,
    )
    .unwrap()
});

/// A branch-agnostic view over the capture groups of [`INLINE_LINK`], which has
/// three parallel top-level branches (angle / link-macro / non-angle). Exactly
/// one branch participates in any given match; this resolves the relevant
/// groups so the replacer doesn't have to special-case the branch numbering
/// everywhere.
///
/// Shared `pub(crate)` (with the string replacer) so the single-pass
/// [`inline_builder`](crate::content::inline_builder) resolves an `INLINE_LINK`
/// match's branch through the *same* group-numbering logic, rather than
/// duplicating that knowledge at its own recognition sink.
pub(crate) struct NormalizedCaps<'c, 't> {
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

    /// Bare (auto-linked) URL; absent in the LINK-MACRO branch, which always
    /// has a trailing attrlist.
    bare: Option<usize>,
}

impl<'c, 't> NormalizedCaps<'c, 't> {
    pub(crate) fn new(caps: &'c Captures<'t>) -> Self {
        if caps.get(1).is_some() {
            // ANGLE branch.
            NormalizedCaps {
                caps,
                is_angle: true,
                prefix: 1,
                scheme: 2,
                target: 3,
                attrlist: 4,
                angle_url: Some(5),
                bare: Some(6),
            }
        } else if caps.get(8).is_some() {
            // LINK-MACRO branch: a `link:` prefix always paired with a trailing
            // attrlist, so there is no bare-link group.
            NormalizedCaps {
                caps,
                is_angle: false,
                prefix: 8,
                scheme: 9,
                target: 10,
                attrlist: 11,
                angle_url: None,
                bare: None,
            }
        } else {
            // NON-ANGLE branch.
            NormalizedCaps {
                caps,
                is_angle: false,
                prefix: 12,
                scheme: 13,
                target: 14,
                attrlist: 15,
                angle_url: None,
                bare: Some(16),
            }
        }
    }

    /// Whether the ANGLE branch matched (its prefix was a leading `&lt;`).
    pub(crate) fn is_angle(&self) -> bool {
        self.is_angle
    }

    /// Whether the LINK-MACRO branch matched (a literal `link:` prefix paired
    /// with a trailing `[…]`). The single-pass builder leaves this branch to
    /// its `link:`/`mailto:` pass, which produces the identical node.
    pub(crate) fn is_link_macro(&self) -> bool {
        self.prefix == 8
    }

    fn prefix(&self) -> &'t str {
        self.caps.get(self.prefix).map_or("", |m| m.as_str())
    }

    /// The boundary-prefix capture (a leading `&lt;`, a `link:`, or the single
    /// boundary character before a non-angle URL), if it participated.
    pub(crate) fn prefix_str(&self) -> &'t str {
        self.prefix()
    }

    /// The scheme capture (`https://`, `\https://`, …) as a [`Match`], so a
    /// caller can recover its source offset.
    pub(crate) fn scheme_match(&self) -> Option<Match<'t>> {
        self.caps.get(self.scheme)
    }

    pub(crate) fn target(&self) -> Option<Match<'t>> {
        self.caps.get(self.target)
    }

    pub(crate) fn attrlist(&self) -> Option<Match<'t>> {
        self.caps.get(self.attrlist)
    }

    /// The URL captured between an angle-bracketed link's `&lt;` and `&gt;`
    /// (the ANGLE branch's `<url>` alternative), if that alternative
    /// participated. `None` for the branch's two other alternatives (a
    /// bracketed attribute list, or an unterminated bare URL) and for both
    /// non-angle branches.
    ///
    /// Shared `pub(crate)` (with the string replacer) so the single-pass
    /// [`inline_builder`](crate::content::inline_builder) tells the ANGLE
    /// branch's three alternatives apart through the *same* group-numbering
    /// logic the replacer uses.
    pub(crate) fn angle_url(&self) -> Option<Match<'t>> {
        self.angle_url.and_then(|g| self.caps.get(g))
    }

    pub(crate) fn bare(&self) -> Option<Match<'t>> {
        self.bare.and_then(|g| self.caps.get(g))
    }
}

/// Matches an inline link (`link:target[…]`) or `mailto:` macro.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes the link and
/// `mailto:` macros with the *exact* same pattern this string step matches
/// with, changing only the recognition *sink* (a node instead of rendered
/// markup).
pub(crate) static INLINE_LINK_MACRO: LazyLock<Regex> = LazyLock::new(|| {
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
            ()                  #   capture group 2: empty target
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

/// This function is used in cases when the attrlist can be mixed with the text
/// of a macro. If no attributes are detected aside from the first positional
/// attribute, and the first positional attribute matches the attrlist, then the
/// original text is returned.
///
/// Precondition: Any new-line characters (`\n`) must be replaced with spaces
/// prior to calling this function.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) can parse a link's
/// attribute-list-bearing display text with the *exact* same interpretation
/// this string step uses, changing only the recognition *sink* (a node field
/// instead of a borrow of the node's own attribute list).
pub(crate) fn extract_attributes_from_text<'src>(
    text: Span<'src>,
    parser: &Parser,
    default_text: Option<&str>,
) -> (String, Attrlist<'src>) {
    let attrlist_maw = Attrlist::parse(text, parser, AttrlistContext::Inline);
    let attrs = attrlist_maw.item.item;

    if let Some(resolved_text) = attrs.nth_attribute(1) {
        // If the resolved text is unchanged from the input — i.e. the attribute
        // list parse produced a single positional value equal to the whole text
        // and split nothing off as a named attribute — clear the attributes and
        // return the text unparsed. This matches Asciidoctor's
        // `extract_attributes_from_text` (substitutors.rb) and is what makes a
        // macro nested inside a link/xref's text (e.g. `link[image:...[]]`)
        // survive intact: the already-rendered inner macro output happens to
        // contain `=` and `"` characters, but is not a real attribute list.
        if resolved_text.value() == text.data() {
            let empty_attrs = Attrlist::parse(Span::default(), parser, AttrlistContext::Inline)
                .item
                .item;
            (text.data().to_owned(), empty_attrs)
        } else {
            (resolved_text.value().to_owned(), attrs)
        }
    } else {
        let default_text = default_text.map(|s| s.to_string());
        (default_text.unwrap_or_default(), attrs)
    }
}

// Ruby CGI.escape (Ruby 2.5+) leaves only A-Z a-z 0-9 _.-~ unescaped and
// encodes everything else; notably `*` is escaped to `%2A` while `~` is not.
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
    .add(b'*') // asterisk must be escaped (CGI.escape emits %2A)
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

/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) can encode a `mailto:`
/// subject/body into the target exactly as this string step does when
/// recognizing a `mailto:` macro's own comma-delimited attribute-list text.
pub(crate) fn encode_uri_component(s: &str) -> String {
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
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes bare e-mail
/// auto-links with the *exact* same pattern this string step matches with,
/// changing only the recognition *sink* (a node instead of rendered markup).
pub(crate) static INLINE_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
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
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes a
/// bibliography anchor with the *exact* same pattern this string step matches
/// with, changing only the recognition *sink* (an
/// [`Anchor`](crate::inlines::Anchor) node whose `is_bibliography` is set,
/// instead of rendered markup). Group 1 is the label and group 2 its optional
/// xreftext.
///
/// ## Examples
///
/// * `[[[label]]]`
/// * `[[[label,xreftext]]]`
pub(crate) static INLINE_BIBLIO_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
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

/// Matches an anchor (i.e., id + optional reference text) in the flow of text.
///
/// ##Examples
///
/// * `[[idname]]`
/// * `[[idname,Reference Text]]`
/// * `anchor:idname[]`
/// * `anchor:idname[Reference Text]`
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes inline anchors
/// with the *exact* same pattern this string step matches with, changing only
/// the recognition *sink* (an [`Anchor`](crate::inlines::Anchor) node instead
/// of rendered markup). Group 1 is the optional escape backslash, groups 2/3
/// the shorthand id and its optional reference text, and groups 4/5 the
/// `anchor:id[…]` macro id and its optional reference text.
pub(crate) static INLINE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
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

/// Matches a cross-reference, in either the double-angle-bracket shorthand or
/// the `xref:` macro form.
///
/// Note that the special-characters substitution runs before macros, so by this
/// point `<<` and `>>` have already become `&lt;&lt;` and `&gt;&gt;`.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) recognizes
/// cross-references with the *exact* same pattern this string step matches
/// with, changing only the recognition *sink* (a node instead of a deferred
/// placeholder). Group 1 is the optional escape backslash, group 2 the
/// shorthand's inner text, group 3 the `xref:` macro target, and group 4 the
/// macro's bracketed text.
///
/// ## Examples
///
/// * `<<idname>>` (seen here as `&lt;&lt;idname&gt;&gt;`)
/// * `<<idname,Reference Text>>`
/// * `xref:idname[]`
/// * `xref:idname[Reference Text]`
pub(crate) static INLINE_XREF: LazyLock<Regex> = LazyLock::new(|| {
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

/// Reads the document-wide `xrefstyle` attribute as an [`XrefStyle`].
///
/// An unset attribute yields `None` (the target's reftext is used verbatim). A
/// set-but-empty value (`:xrefstyle:`) and any unrecognized value both resolve
/// to [`XrefStyle::Basic`], mirroring Asciidoctor.
pub(crate) fn document_xrefstyle(parser: &Parser) -> Option<XrefStyle> {
    match parser.attribute_value("xrefstyle") {
        InterpretedValue::Value(value) => Some(XrefStyle::parse(&value)),
        InterpretedValue::Set => Some(XrefStyle::Basic),
        InterpretedValue::Unset => None,
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
/// is not matched again; the `regex` crate has no look-ahead, so the builder's
/// footnote family (see `inline_builder::footnotes`) re-creates that guard by
/// inspecting the text that follows the match.
///
/// [footnote]: https://docs.asciidoctor.org/asciidoc/latest/macros/footnote/
pub(crate) static INLINE_FOOTNOTE_MACRO: LazyLock<Regex> = LazyLock::new(|| {
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

/// Sniffs a leading URI scheme (e.g. `https://`), used to strip it from a link's
/// display text under the `hide-uri-scheme` document attribute.
///
/// Shared `pub(crate)` so the single-pass
/// [`inline_builder`](crate::content::inline_builder) reproduces the same
/// `hide-uri-scheme` display text the string step's link macro computes.
pub(crate) static URI_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"^\p{alpha}[\p{alpha}\p{digit}.+-]+:/{0,2}"#).unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    //! This test suite fills in a few coverage gaps after doing spec-driven
    //! development (SDD) for macro parsing.

    mod encode_uri_component {
        use super::super::encode_uri_component;

        // Mirrors Asciidoctor's `Helpers.encode_uri_component`
        // (helpers_test.rb, `context 'URI Encoding'`): non-word characters are
        // percent-encoded, including `*` → `%2A`. `encode_uri_component` is
        // private and reachable in production only via `mailto:` subject/body
        // rendering, where the special-characters substitution rewrites the
        // input first; this exercises the helper's contract directly.
        #[test]
        fn encodes_non_word_characters_generally() {
            assert_eq!(
                encode_uri_component(" !*/%&?\\="),
                "%20%21%2A%2F%25%26%3F%5C%3D"
            );
        }

        // `-` and `.` are left unencoded, as is `~` on Ruby 2.5+ (which this
        // crate matches).
        #[test]
        fn leaves_select_non_word_characters_unencoded() {
            assert_eq!(encode_uri_component("-.~"), "-.~");
        }
    }

    mod inline_image_macro {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        fn apply_macros(source: &'static str) -> Content<'static> {
            let mut content = Content::from(crate::Span::new(source));
            SubstitutionGroup::Custom(vec![SubstitutionStep::Macros]).apply(
                &mut content,
                &Parser::default(),
                None,
            );
            content
        }

        // An `image:`/`icon:` macro written without a target is not an image
        // macro (Asciidoctor's `InlineImageMacroRx` requires one), so the text
        // is left exactly as the author typed it. This used to panic: the
        // target capture group was optional but indexed unconditionally.
        #[test]
        fn image_macro_without_target_is_left_literal() {
            let content = apply_macros("image:[]");
            assert_eq!(content.rendered, CowStr::Borrowed("image:[]"));
        }

        #[test]
        fn icon_macro_without_target_is_left_literal() {
            let content = apply_macros("icon:[]");
            assert_eq!(content.rendered, CowStr::Borrowed("icon:[]"));
        }

        // Same, with alt text in the brackets and surrounding text: the macro
        // is not recognized, and no part of the line is consumed.
        #[test]
        fn image_macro_without_target_but_with_alt_text_is_left_literal() {
            let content = apply_macros("See image:[alt] here.");
            assert_eq!(content.rendered, CowStr::Borrowed("See image:[alt] here."));
        }

        // Only the *trailing* portion of the target is optional, so a
        // one-character target is a valid macro.
        #[test]
        fn single_character_target_is_an_image() {
            let content = apply_macros("image:a[Alt]");
            assert_eq!(
                content.rendered,
                CowStr::Boxed(
                    r#"<span class="image"><img src="a" alt="Alt"></span>"#
                        .to_string()
                        .into_boxed_str()
                )
            );
        }
    }

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
                            rendered: "<a href=\"https://chat.asciidoc.org\" class=\"bare button\" target=\"_blank\" rel=\"nofollow noopener\">chat.asciidoc.org</a>",
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
                .child_blocks()
                .next()
                .unwrap()
                .rendered_html_content()
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
                .child_blocks()
                .next()
                .unwrap()
                .rendered_html_content()
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
                            rendered: "<a href=\"mailto:?subject=Subscribe%20me\"></a>",
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
        fn duplicate_inline_anchor_records_warning() {
            let doc = Parser::default()
                .parse("[#in-use]\nA paragraph with an id.\n\nAnother paragraph\n[[in-use]]that uses an id\nwhich is already in use.\n");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings.first().unwrap().warning,
                WarningType::DuplicateId("in-use".to_string())
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

            // The inner `[[mid]]` is preceded by a `[`, so — like Asciidoctor's
            // inline-anchor scan (`InlineAnchorScanRx`) — the id is rendered but
            // not registered in the catalog, neither as a bibliography anchor nor
            // as a normal one. See #769.
            assert!(doc.catalog().get_ref("mid").is_none());
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
