//! The passthrough-extraction substitution step.

use super::{
    attribute_refs::apply_attribute_references,
    char_replacements::apply_character_replacements,
    macros::{
        MacroMatch, MacroMatchKind, apply_macros, image::range_is_verbatim, rebuild_macro_level,
    },
    post_replacements::apply_post_replacements,
    quotes::{Piece, apply_quotes, attributes_of, build_match_string, source_slice},
    special_chars::apply_special_characters,
};
use crate::{
    Parser, Span,
    content::{Content, INLINE_PASS, INLINE_PASS_MACRO, SubstitutionGroup},
    inlines::{InlineNode, SpanForm, StyleVariant, Styled},
    strings::CowStr,
};

/// The passthrough-extraction step, as a node transducer: replaces each
/// recognized passthrough with a [`Raw`](InlineNode::Raw) leaf and leaves
/// everything else as the whole-source seed [`Text`](InlineNode::Text) node,
/// for [`apply_special_characters`] and the later steps to refine.
///
/// This is the **first** step [`build`](super::build) runs – mirroring
/// [`Passthroughs::extract_from`](crate::content::Passthroughs::extract_from),
/// which the string pipeline runs *before* its own step loop – so a
/// passthrough's content is never touched by specialcharacters, quotes,
/// replacements, or macros: it is a leaf, and every later step's
/// [`build_match_string`] already treats a node it does not specifically
/// handle (an already-built [`Styled`] span, and now a
/// [`Raw`](InlineNode::Raw) leaf) as a
/// single opaque placeholder.
///
/// It reuses the string pipeline's *exact* recognition –
/// [`INLINE_PASS_MACRO`] is now shared `pub(crate)` – so only the recognition
/// *sink* differs (§4.1). Two forms fold through a [`Raw`](InlineNode::Raw)
/// node whose `value`
/// is the *content itself*, since their substitution list applies nothing:
/// the triple-plus (`+++text+++`) and bare `pass:[…]` macro (with no
/// substitution list) both resolve to [`SubstitutionGroup::None`], and the
/// content borrows `'src` directly (a `pass:[…]` body unescapes an escaped
/// `\]`, as every other macro family's bracket content does, which makes the
/// unescaped case owned). The double-plus (`++text++`) and double-dollar
/// (`$$text$$`) forms resolve to [`SubstitutionGroup::Verbatim`] (special
/// characters only); rather than hand-escape `<`/`>`/`&`, their content is
/// run through the real substitution pipeline
/// ([`passthrough_text`]) so a custom
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)'s
/// escaping is honored, exactly as it would be for the string pipeline's own
/// restore step – the cost is an owned [`Raw`](InlineNode::Raw) value rather
/// than a `'src` borrow, since the pipeline's output is not guaranteed to
/// coincide with the source.
///
/// An **attribute-list-prefixed** passthrough (`[quotes]++text++`,
/// `` [x-]`text` ``, `[attrs]+text+`) folds through a [`Styled`] node instead:
/// [`build_attrlisted_passthrough_node`] and
/// [`build_bare_attrlisted_passthrough_node`] parse the attrlist the same way
/// an attributed quote does ([`attributes_of`]) and wrap the body – itself a
/// `Raw` leaf under `SubstitutionGroup::None`/`Verbatim`, unless the legacy
/// `x-` compatibility marker switches it to a full `Normal`-order subtree
/// ([`apply_normal_subs`]) – in `Code` (monospace) or `Unquoted`, mirroring
/// `PassthroughRestoreReplacer`'s own `render_quoted_substitution` call for a
/// stored passthrough whose `type_` is `Some`. This runs as a **second pass**
/// ([`apply_bare_attrlisted_pass_level`]) after the delimited forms above,
/// mirroring `Passthroughs::extract_from`'s own order (`INLINE_PASS_MACRO`
/// before [`INLINE_PASS`]).
///
/// A **`pass:` macro carrying an explicit substitution list** (`pass:c,q[…]`,
/// whose content would need a richer subtree than a single `Raw` leaf – the
/// same reason a footnote's content is structured children rather than a
/// literal value) and the **bare unconstrained form** (`+text+`, no
/// attribute list – its "must not follow a word" boundary needs a lookbehind
/// Rust's regex engine cannot express, which the string replacer works
/// around with a retry loop this increment does not reproduce) remain
/// deferred. So does the closely related **"prohibited prefix"** the string
/// replacer's own retry loop protects (an attribute-list-prefixed bare match
/// immediately preceded by `\`, `:`, or `;`): rather than reproduce the
/// retry, such a match is simply left unrecognized – a documented divergence.
/// Inline STEM (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) is an implicit
/// passthrough too, but folds through its own [`Stem`](InlineNode::Stem) node
/// rather than `Raw`, so it is recognized by its own step,
/// [`apply_stem`](super::stem_step::apply_stem), run immediately after this
/// one (mirroring `Passthroughs::extract_from`, which extracts STEM macros
/// last, after both passthrough passes). This step is **additive**: nothing
/// is wired into the parse path.
///
/// One more attribute-list-prefixed corner case is deferred: an **escaped
/// bracket** (`\[attrs]++text++`) unescapes to a literal `[attrs]` prefix
/// *and* still recognizes the delimited text as an ordinary (non-attrlisted)
/// passthrough – a kept-literal-prefix-with-one-dropped-char, plus a node for
/// the remainder, a shape neither [`MacroMatchKind`] variant expresses. Left
/// unrecognized (a documented divergence); see
/// `an_escaped_attrlist_bracket_is_a_documented_divergence`.
///
/// The already-deferred bare-form boundary shows up once more, indirectly: an
/// **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops its
/// backslash and keeps the delimited text literal here, but the string
/// pipeline's *second* extraction pass ([`INLINE_PASS`]) re-scans that same
/// de-escaped text and consumes its leading `+++`/`++` as a bare passthrough
/// wrapping a shorter run – so these two escape forms are pinned as
/// divergences (`an_escaped_triple_plus_stays_literal`,
/// `an_escaped_double_plus_stays_literal`) rather than folded into the main
/// parity corpus. An escaped `$$…$$` or `pass:[…]` has no such residue and
/// stays parity, since [`INLINE_PASS`] never matches `$$` or `pass:` syntax.
/// An escaped attribute-list-prefixed *delimiter* (`[attrs]\++text++`), by
/// contrast, is **not** a divergence: the delimiter's own `Unescape` leaves
/// literal, unopaqued text behind, so the bare-form second pass legitimately
/// re-recognizes it exactly as the string pipeline's own second regex pass
/// does – parity, not residue.
///
/// [`INLINE_PASS`]: crate::content::passthroughs
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer):
/// crate::parser::InlineSubstitutionRenderer
pub(super) fn apply_passthroughs<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let nodes = apply_pass_macro_level(nodes, root, parser);

    // The bare-form pass runs second, mirroring `Passthroughs::extract_from`'s
    // own order: the string pipeline runs `INLINE_PASS_MACRO` first, then
    // `INLINE_PASS` over what it left behind, so a construct the macro pass
    // already replaced (now an opaque placeholder in the rebuilt match
    // string) is untouched by this second pass.
    apply_bare_attrlisted_pass_level(nodes, root, parser)
}

/// The `INLINE_PASS_MACRO` pass: `+++…+++`, `++…++`, `$$…$$`, and `pass:[…]`,
/// with or without an attribute list ahead of the delimiters.
fn apply_pass_macro_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring `Passthroughs::extract_from`'s own guard: a
    // recognized form always contains `++`, `$$`, or (for `pass:`) `ss:`.
    if !(s.contains("++") || s.contains("$$") || s.contains("ss:")) {
        return nodes;
    }

    let matches = find_passthrough_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// The `INLINE_PASS` pass: the attribute-list-prefixed bare forms
/// (`` [x-]`text` ``, `[attrs]+text+`). The bare unconstrained form with no
/// attribute list (`+text+`) is deferred – see
/// [`find_bare_attrlisted_matches`].
fn apply_bare_attrlisted_pass_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring `Passthroughs::extract_from`'s own guard for
    // `INLINE_PASS`.
    if !(s.contains('+') || s.contains("-]")) {
        return nodes;
    }

    let matches = find_bare_attrlisted_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every passthrough at this level, skipping the deferred forms
/// [`apply_passthroughs`] documents: a `pass:` macro carrying an explicit
/// substitution list, and an attribute-list-prefixed match whose *bracket* is
/// escaped (`\[attrs]++text++`) – the one remaining documented divergence. A
/// *delimiter* escape (`[attrs]\++text++`) is not deferred: it becomes an
/// [`Unescape`](MacroMatchKind::Unescape) that drops one backslash and leaves
/// the rest literal, exactly like an unattrlisted delimiter escape.
fn find_passthrough_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_PASS_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Only a wholly-verbatim match can slice its content from `'src`; a
        // match crossing an already-recognized construct cannot occur here –
        // this is the very first step – but the check is kept for the same
        // reason every other family keeps it: a future caller of this
        // function over a non-seed level must not silently mis-slice.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        // A `pass:` macro carrying an explicit substitution list
        // (`pass:c,q[…]`) is deferred.
        if caps.get(14).is_some() {
            continue;
        }

        if let Some(attrlist) = caps.get(2) {
            // An attribute list ahead of the delimiters (`[quotes]++text++`).
            let escape_count = caps.get(3).map_or(0, |m| m.len());

            if escape_count > 0 {
                // `[attrs]\++text++`: the delimiter escape drops one
                // backslash and the whole match – attrlist brackets included
                // – stays literal here, mirroring `handle_quoted_text`'s
                // `escape_count > 0` branch, which never builds a passthrough
                // either. The bare-form second pass
                // (`apply_bare_attrlisted_pass_level`) then legitimately
                // re-scans this now-literal, unopaqued text and may recognize
                // its own (different) match in it – exactly what the string
                // pipeline's own second regex pass does, so this is parity,
                // not a divergence.
                #[allow(clippy::unwrap_used)]
                let group3 = caps.get(3).unwrap();

                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: group3.start(),
                    },
                    full,
                });

                continue;
            }

            if &caps[1] == "\\" {
                // `\[attrs]++text++`: the bracket unescapes to a literal
                // `[attrs]`, but the delimited text is still recognized as
                // an *ordinary* (non-attrlisted) passthrough – a
                // kept-literal-prefix-plus-node shape neither
                // `MacroMatchKind` variant expresses. Deferred; see
                // `an_escaped_attrlist_bracket_is_a_documented_divergence`.
                continue;
            }

            let node =
                build_attrlisted_passthrough_node(&caps, &full, attrlist, pieces, root, parser);

            matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            });

            continue;
        }

        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_passthrough_node(&caps, &full, pieces, root, parser);

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Finds every attribute-list-prefixed bare passthrough at this level (the
/// two [`INLINE_PASS`] options that carry an attribute list –
/// `` [x-]`text` `` and `[attrs]+text+`), skipping the bare unconstrained form
/// with no attribute list (`+text+`, deferred to a later increment: its
/// "must not follow a word" boundary needs a lookbehind Rust's regex engine
/// cannot express).
///
/// [`INLINE_PASS`] also needs a lookbehind the string replacer works around
/// with a retry loop (`InlinePassReplacer`'s "prohibited prefix" check): a
/// match immediately preceded by `\`, `:`, or `;` is not really a
/// passthrough. This increment does not reproduce that retry; instead it
/// simply leaves such a match unrecognized, a documented divergence pinned by
/// a test.
fn find_bare_attrlisted_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_PASS.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        let is_backtick = caps.get(1).is_some();
        let is_plus_attrlisted = caps.get(3).is_some();

        // The bare unconstrained form (no attribute list) is deferred.
        if !is_backtick && !is_plus_attrlisted {
            continue;
        }

        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        if full
            .start
            .checked_sub(1)
            .and_then(|i| s.as_bytes().get(i))
            .is_some_and(|b| matches!(b, b'\\' | b':' | b';'))
        {
            continue;
        }

        if is_plus_attrlisted {
            let escape_count = caps.get(4).map_or(0, |m| m.len());

            if escape_count > 0 {
                // `[attrs]\+text+`: honor the escape of the formatting mark –
                // one backslash drops, the rest (attrlist brackets included)
                // stays literal, mirroring `InlinePassReplacer`'s own
                // `escape_count > 0` branch, which never builds a
                // passthrough here.
                #[allow(clippy::unwrap_used)]
                let group4 = caps.get(4).unwrap();

                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: group4.start(),
                    },
                    full,
                });

                continue;
            }
        }

        let node =
            build_bare_attrlisted_passthrough_node(&caps, &full, is_backtick, pieces, root, parser);

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Builds one [`Raw`](InlineNode::Raw) node from a verbatim, unescaped
/// passthrough match – see [`apply_passthroughs`] for how each delimiter form
/// maps to its `value`.
fn build_passthrough_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    if let Some(m) = caps.get(5) {
        // `+++text+++`: `SubstitutionGroup::None` applies nothing, so the
        // content is genuinely raw and borrows `'src` directly.
        let content = source_slice(pieces, m.start()..m.end(), root);

        return InlineNode::Raw {
            value: CowStr::from(content.data()),
            location,
        };
    }

    if let Some(m) = caps.get(8).or_else(|| caps.get(11)) {
        // `++text++` / `$$text$$`: `SubstitutionGroup::Verbatim` applies only
        // special characters. Computed through the real substitution
        // pipeline (rather than hand-escaping `<`/`>`/`&`) so a custom
        // `InlineSubstitutionRenderer`'s escaping is honored.
        let content = source_slice(pieces, m.start()..m.end(), root);
        let value = passthrough_text(content.data(), &SubstitutionGroup::Verbatim, parser);

        return InlineNode::Raw {
            value: CowStr::from(value),
            location,
        };
    }

    // The bare `pass:[…]` macro (no explicit substitution list):
    // `SubstitutionGroup::None` applies nothing, and an escaped closing
    // bracket (`\]`) unescapes, mirroring the string replacer's
    // `text.replace("\\]", "]")` – the same treatment every other macro
    // family's bracket content gets.
    #[allow(clippy::unwrap_used)]
    let m = caps.get(15).unwrap();
    let content = source_slice(pieces, m.start()..m.end(), root);
    let raw = content.data();

    let value = if raw.contains("\\]") {
        CowStr::from(raw.replace("\\]", "]"))
    } else {
        CowStr::from(raw)
    };

    InlineNode::Raw { value, location }
}

/// Builds one [`Styled`] node from a verbatim, unescaped, attribute-listed
/// `INLINE_PASS_MACRO` match (`[attrs]+++text+++`, `[attrs]++text++`,
/// `[attrs]$$text$$`) – the delimited half of the attribute-list-prefixed
/// forms this increment recognizes (see [`build_bare_attrlisted_passthrough_node`]
/// for the bare half). Folds through the same `render_quoted_substitution`
/// `PassthroughRestoreReplacer` calls when its stored passthrough carries a
/// `type_`/`attrlist`, so the output is byte-for-byte identical.
///
/// Only the `++` boundary can trigger the legacy `x-` compatibility marker
/// (`handle_quoted_text`'s `old_behavior`, see [`split_old_behavior_attrlist`]);
/// `+++`/`$$` always keep the attrlist as written and never switch to the
/// `Normal` substitution group.
fn build_attrlisted_passthrough_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    attrlist: regex::Match<'_>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);
    let attrlist_span = source_slice(pieces, attrlist.start()..attrlist.end(), root);

    let boundary = caps
        .get(4)
        .or_else(|| caps.get(7))
        .or_else(|| caps.get(10))
        .map_or("", |m| m.as_str());

    // Each delimited alternative's body group is mandatory (its `(.*?)`
    // participates, possibly empty, whenever that alternative matches at
    // all) – exactly one of groups 5/8/11 is therefore always `Some` here,
    // never a genuinely absent capture.
    #[allow(clippy::unwrap_used)]
    let body_m = caps
        .get(5)
        .or_else(|| caps.get(8))
        .or_else(|| caps.get(11))
        .unwrap();
    let body_span = source_slice(pieces, body_m.start()..body_m.end(), root);

    let (attrlist_span, old_behavior) = if boundary == "++" {
        split_old_behavior_attrlist(attrlist_span)
    } else {
        (attrlist_span, false)
    };

    let variant = if old_behavior {
        StyleVariant::Code
    } else {
        StyleVariant::Unquoted
    };

    let children = if old_behavior {
        apply_normal_subs(body_span, parser)
    } else {
        let subs = if boundary == "+++" {
            SubstitutionGroup::None
        } else {
            SubstitutionGroup::Verbatim
        };

        let value = passthrough_text(body_span.data(), &subs, parser);

        vec![InlineNode::Raw {
            value: CowStr::from(value),
            location: body_span,
        }]
    };

    let (id, roles, attrs) = attributes_of(attrlist_span, parser);

    InlineNode::Styled(Styled {
        variant,
        form: SpanForm::Unconstrained,
        id,
        roles,
        attrs,
        children,
        location,
    })
}

/// Builds one [`Styled`] node from a verbatim, unescaped, attribute-listed
/// bare [`INLINE_PASS`] match – either the backtick form (`` [x-]`text` ``,
/// `is_backtick`) or the plus form (`[attrs]+text+`).
///
/// The backtick form's attrlist is *always* `x-`-eligible – the regex itself
/// requires it (`` INLINE_PASS ``'s option 1 only matches `[x-]` or
/// `[… x-]`) – but its format mark (`` ` ``) keeps `subs` at `Verbatim`
/// regardless, mirroring `InlinePassReplacer`'s `format_mark != '`'` guard:
/// only the plus form's `old_behavior` switches to the `Normal` group.
fn build_bare_attrlisted_passthrough_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    is_backtick: bool,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    let (attrlist, body) = if is_backtick {
        (caps.get(1), caps.get(2))
    } else {
        (caps.get(3), caps.get(5))
    };

    #[allow(clippy::unwrap_used)]
    let attrlist_m = attrlist.unwrap();
    let attrlist_span = source_slice(pieces, attrlist_m.start()..attrlist_m.end(), root);

    // Both the backtick body (group 2, requiring at least one non-space
    // character) and the plus body (group 5) are mandatory captures of
    // whichever alternative matched – never a genuinely absent one.
    #[allow(clippy::unwrap_used)]
    let body_m = body.unwrap();
    let body_span = source_slice(pieces, body_m.start()..body_m.end(), root);

    let (attrlist_span, old_behavior) = split_old_behavior_attrlist(attrlist_span);

    let variant = if old_behavior {
        StyleVariant::Code
    } else {
        StyleVariant::Unquoted
    };

    let children = if old_behavior && !is_backtick {
        apply_normal_subs(body_span, parser)
    } else {
        let value = passthrough_text(body_span.data(), &SubstitutionGroup::Verbatim, parser);

        vec![InlineNode::Raw {
            value: CowStr::from(value),
            location: body_span,
        }]
    };

    let (id, roles, attrs) = attributes_of(attrlist_span, parser);

    InlineNode::Styled(Styled {
        variant,
        form: SpanForm::Unconstrained,
        id,
        roles,
        attrs,
        children,
        location,
    })
}

/// Splits an old-behavior-eligible attrlist span into its final attrlist body
/// and whether the legacy `x-` compatibility marker was present – mirroring
/// the string replacer's own check (`handle_quoted_text` and
/// `InlinePassReplacer` both apply it identically): an attrlist of exactly
/// `x-` clears entirely; one *ending* in ` x-` drops that suffix; anything
/// else is kept as written and is not old-behavior.
fn split_old_behavior_attrlist(attrlist: Span<'_>) -> (Span<'_>, bool) {
    let text = attrlist.data();

    if text == "x-" {
        (attrlist.slice(0..0), true)
    } else if let Some(stripped) = text.strip_suffix(" x-") {
        (attrlist.slice(0..stripped.len()), true)
    } else {
        (attrlist, false)
    }
}

/// Runs `text` through the *normal* substitution order (special characters,
/// quotes, attribute references, character replacements, macros, post
/// replacement – [`SubstitutionGroup::Normal`]'s own step list) and returns
/// the resulting node subtree, for the legacy `x-` compatibility marker's
/// `Normal`-group passthrough body (see [`split_old_behavior_attrlist`]).
///
/// This mirrors `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call
/// for that case, except as a node transducer: `Normal`'s step list excludes
/// [`apply_passthroughs`] and [`apply_stem`](super::apply_stem) (mirroring
/// that passthrough/STEM extraction happens once, ahead of
/// [`SubstitutionGroup::apply`], not inside it), so `text` is threaded
/// through the remaining six steps directly, with itself as the root a
/// child's `location` is sliced from.
fn apply_normal_subs<'src>(text: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
    let nodes = vec![InlineNode::Text {
        value: CowStr::from(text.data()),
        location: text,
    }];

    let nodes = apply_special_characters(nodes);
    let nodes = apply_quotes(nodes, text, parser);
    let nodes = apply_attribute_references(nodes, text, parser);
    let nodes = apply_character_replacements(nodes, text);
    let nodes = apply_macros(nodes, text, parser);
    apply_post_replacements(nodes, text)
}

/// Runs `text` through the real substitution pipeline under `subs`, returning
/// the resulting owned string. Used to compute a [`Raw`](InlineNode::Raw)
/// passthrough's `value` under [`SubstitutionGroup::Verbatim`] – mirroring
/// `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call in the string
/// pipeline's restore step – so the result honors whatever
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)
/// `parser` carries rather than a hand-rolled, always-default escaping.
pub(super) fn passthrough_text(text: &str, subs: &SubstitutionGroup, parser: &Parser) -> String {
    let mut content = Content::from(Span::new(text));
    subs.apply(&mut content, parser, None);
    content.rendered_str().to_string()
}

/// Reports whether `c` is one of the three characters the special-characters
/// substitution acts on.
pub(super) fn is_special(c: char) -> bool {
    matches!(c, '<' | '>' | '&')
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{
            assert_raw, assert_styled, build_src, fold_html, golden_passthroughs, seed,
        },
        apply_passthroughs,
    };
    use crate::{
        HasSpan, Parser, Span,
        inlines::{InlineNode, SpanForm, StyleVariant, Styled},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn apply_passthroughs_is_a_noop_without_passthrough_syntax() {
        // The cheap pre-filter returns the seed unchanged when no `++`, `$$`, or
        // `ss:` substring is present, so ordinary content never pays for the
        // level-building machinery.
        let source = Span::new("plain text, no passthrough syntax here");
        let seeded = seed(source);
        let nodes = apply_passthroughs(seeded.clone(), source, &Parser::default());

        assert_eq!(nodes, seeded);
    }

    #[test]
    fn a_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // In practice `apply_passthroughs` only ever runs on the pristine
        // whole-source seed (it is `build`'s first step, ahead of every node
        // that could make a range non-verbatim), so `range_is_verbatim`'s
        // false branch is defensive – kept for the same reason every other
        // macro family keeps the check. Exercise it directly, feeding a
        // hand-built level whose triple-plus content spans an already-built
        // `Styled` node, to document the intended fallback: the whole match
        // is left unrecognized rather than mis-sliced.
        let location = Span::new("+++");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("+++"),
                location,
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location,
            }),
            InlineNode::Text {
                value: CowStr::from("+++"),
                location,
            },
        ];

        let root = Span::new("+++");
        let result = apply_passthroughs(nodes.clone(), root, &Parser::default());

        assert_eq!(
            result, nodes,
            "a non-verbatim match must be left unrecognized"
        );
    }

    #[test]
    fn triple_plus_borrows_its_content_unescaped() {
        // `SubstitutionGroup::None` applies nothing, so the content is genuinely
        // raw – not even special characters are escaped – and borrows `'src`.
        let nodes = build_src(Span::new("+++<b>*not quotes*</b>+++"));

        assert_eq!(nodes.len(), 1);

        // `location` covers the whole `+++…+++` construct (delimiters
        // included), matching every other macro node; only `value` is the
        // unwrapped content.
        let location = assert_raw(&nodes[0], "<b>*not quotes*</b>");
        assert_eq!(location.data(), "+++<b>*not quotes*</b>+++");

        match &nodes[0] {
            InlineNode::Raw { value, .. } => {
                assert!(
                    matches!(value, CowStr::Borrowed(_)),
                    "triple-plus content should borrow from source, got {value:?}"
                );
            }
            other => panic!("expected Raw, got {other:?}"),
        }
    }

    #[test]
    fn double_plus_and_double_dollar_escape_specials_only() {
        // `SubstitutionGroup::Verbatim` applies only special characters: `<`/`>`
        // are escaped, but `*not quotes*` is left alone (quotes never runs over
        // passthrough content).
        for source in ["++<b>*not quotes*</b>++", "$$<b>*not quotes*</b>$$"] {
            let nodes = build_src(Span::new(source));

            assert_eq!(nodes.len(), 1, "for {source:?}: {nodes:?}");
            assert_raw(&nodes[0], "&lt;b&gt;*not quotes*&lt;/b&gt;");
        }
    }

    #[test]
    fn bare_pass_macro_borrows_unescaped_content() {
        let nodes = build_src(Span::new("pass:[<b>*not quotes*</b>]"));

        assert_eq!(nodes.len(), 1);
        let location = assert_raw(&nodes[0], "<b>*not quotes*</b>");
        assert_eq!(location.data(), "pass:[<b>*not quotes*</b>]");
    }

    #[test]
    fn an_empty_pass_macro_builds_an_empty_raw_node() {
        let nodes = build_src(Span::new("pass:[]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "");
    }

    #[test]
    fn a_pass_macro_unescapes_an_escaped_closing_bracket() {
        // Mirrors the string replacer's `text.replace("\\]", "]")`, the same
        // treatment every other macro family's bracket content gets. The
        // unescape makes the value owned, unlike the no-escape case above.
        let nodes = build_src(Span::new(r"pass:[a\]b]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "a]b");

        match &nodes[0] {
            InlineNode::Raw { value, .. } => {
                assert!(matches!(value, CowStr::Boxed(_)), "got {value:?}");
            }
            other => panic!("expected Raw, got {other:?}"),
        }
    }

    #[test]
    fn an_escaped_triple_plus_stays_literal() {
        // The builder drops the single backslash and keeps `+++text+++`
        // literal, mirroring every other family's `\image:…` escape handling.
        // The string pipeline instead runs a *second* pass (`INLINE_PASS`,
        // the deferred bare `+text+` form) over that same de-escaped text,
        // which re-consumes its leading `+++` as a bare passthrough wrapping
        // a single `+` – so this is a documented divergence, not parity, and
        // stems from the same deferred boundary as the bare unconstrained
        // form.
        let source = r"\+++text+++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "+++text+++");
        assert_ne!(folded, golden_passthroughs(source));
    }

    #[test]
    fn an_escaped_double_plus_stays_literal() {
        // Same divergence as `an_escaped_triple_plus_stays_literal`, one
        // delimiter layer down: de-escaping `\++text++` to `++text++` leaves
        // a leading `++` the deferred bare-form pass also re-consumes.
        let source = r"\++text++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "++text++");
        assert_ne!(folded, golden_passthroughs(source));
    }

    #[test]
    fn an_escaped_pass_macro_stays_literal() {
        let nodes = build_src(Span::new(r"\pass:[x]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "pass:[x]");
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_passthroughs() {
        // For each fixture, folding the single-pass tree (all six steps,
        // passthroughs included) reproduces the string pipeline's output
        // byte-for-byte. This is the differential corpus (design §5.3) that
        // pins the passthrough increment.
        let fixtures = [
            // No passthrough despite passthrough-ish characters.
            "plain text",
            "a colon : and a plus + apart",
            "one + two - not a passthrough",
            // Triple-plus: no substitutions at all.
            "+++<b>bold</b>+++",
            "+++*not quotes*+++",
            "prefix +++<raw/>+++ suffix",
            "+++line one\nline two+++",
            // Double-plus / double-dollar: special characters only.
            "++<b>++",
            "++*not quotes*++",
            "$$<b>$$",
            "$$*not quotes*$$",
            // The bare `pass:[…]` macro.
            "pass:[<b>]",
            "pass:[]",
            r"pass:[a\]b]",
            "pass:[*not quotes*]",
            // Multiple passthroughs, and passthroughs beside ordinary quoted
            // text (proving the surrounding text is still substituted
            // normally).
            "a +++x+++ and a ++y++ and a $$z$$ and a pass:[w]",
            "*bold* +++<raw/>+++ *more bold*",
            // Escapes. The `+++`/`++` forms are handled elsewhere (see
            // `an_escaped_triple_plus_stays_literal` and
            // `an_escaped_double_plus_stays_literal`), since de-escaping them
            // can leave a residue the deferred bare `+text+` pass would also
            // consume – a documented divergence, not parity.
            r"\$$text$$",
            r"\pass:[x]",
            // Attribute-list-prefixed delimited forms (`INLINE_PASS_MACRO`'s
            // own attrlist branch): a role, an id, a quoted role, and
            // multiple roles plus an id.
            "[.role]++text++",
            "[.role]+++text+++",
            "[.role]$$text$$",
            "[#anchor]++text++",
            "['quoted role']++text++",
            "[.a.b#id]++text++",
            // `<`/`>`/`&` and quote-ish content inside the body: `+++`/`pass:`
            // stay raw, `++`/`$$` escape specials only, and quotes never run
            // over the body either way.
            "[.role]+++<b>*x*</b>+++",
            "[.role]++<b>*x*</b>++",
            "[.role]$$<b>*x*</b>$$",
            // The legacy `x-` compatibility marker: only the `++` boundary
            // switches to monospace with the full `Normal` substitution
            // order (quotes, macros, and all); `+++`/`$$` keep `x-` as a
            // literal role instead.
            "[x-]++text++",
            "[x-]++*bold* and _em_++",
            "[x-]++image:x.png[X]++",
            "[method x-]++text++",
            "[method x-]++save()++",
            "[ x-]++text++",
            "[x -]++text++",
            "[x-]+++*bold*+++",
            "[x-]$$*bold*$$",
            // Attribute-list-prefixed bare forms (`INLINE_PASS`): the
            // backtick form is always monospace under `Verbatim` subs
            // (regardless of its attrlist body), and the plus form behaves
            // like the delimited `++` boundary above, `x-` included.
            "[x-]`leave it alone`",
            "[method x-]`leave it alone`",
            "[x-]`just *mono*`",
            "[.role]+text+",
            "[x-]+save()+",
            "[method x-]+*bold*+",
            "[x-]+{missing-attr}+",
            "[x-]+<b>*x*</b>+",
            // A passthrough beside ordinary flow, and one spanning a
            // newline.
            "a [.role]++text++ b",
            "a [x-]`code` b",
            "multi\nline [x-]++text\nmore++ end",
            // A delimiter escape after an attribute list drops one backslash
            // and leaves the rest literal – which the bare-form second pass
            // then legitimately re-recognizes as its own (different) match,
            // exactly as the string pipeline's own second regex pass does.
            r"[.role]\++text++",
            // The bare-plus form's own delimiter escape: dropped backslash,
            // literal remainder, no further pass to re-scan a residue.
            r"[attrs]\+text+",
        ];

        for source in fixtures {
            let nodes = build_src(Span::new(source));
            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

            assert_eq!(
                folded,
                golden_passthroughs(source),
                "mismatch for {source:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_prefixed_delimited_passthrough_is_a_styled_span() {
        // `[.role]++text++` splices an attribute list ahead of the
        // delimiters, so it folds through a `Styled` node (`Unquoted`,
        // `Unconstrained`) whose single `Raw` child carries the passthrough
        // body – not a plain `Raw` leaf.
        let nodes = build_src(Span::new("[.role]++text++"));

        assert_eq!(nodes.len(), 1);
        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);

        assert_eq!(children.len(), 1);
        assert_raw(&children[0], "text");

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.roles, vec![CowStr::from("role")]);
                assert!(styled.attrs.is_some(), "the attribute list is retained");
            }

            other => panic!("expected Styled, got {other:?}"),
        }

        assert_eq!(nodes[0].span().data(), "[.role]++text++");
    }

    #[test]
    fn an_id_only_attrlist_is_captured() {
        let nodes = build_src(Span::new("[#anchor]++text++"));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.id.as_deref(), Some("anchor"));
                assert!(styled.roles.is_empty());
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn the_plus_plus_boundary_switches_special_characters_only() {
        // `[.role]++<b>++`: `SubstitutionGroup::Verbatim` applies only special
        // characters – the same treatment the unattrlisted `++…++` form gets.
        let nodes = build_src(Span::new("[.role]++<b>++"));

        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw(&children[0], "&lt;b&gt;");
    }

    #[test]
    fn the_triple_plus_boundary_applies_no_substitutions() {
        // `[.role]+++<b>+++`: `SubstitutionGroup::None` applies nothing, so
        // the body is genuinely raw, unlike the `++`/`$$` boundaries.
        let nodes = build_src(Span::new("[.role]+++<b>+++"));

        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw(&children[0], "<b>");
    }

    #[test]
    fn the_legacy_x_dash_marker_switches_to_monospace_and_normal_subs() {
        // `[x-]++*bold*++`: the `++` boundary's legacy compatibility marker
        // switches the variant to `Code` (monospace) and the body to the full
        // `Normal` substitution order – quotes included – unlike the
        // ordinary attrlist case above, whose body is always a single `Raw`
        // leaf.
        let nodes = build_src(Span::new("[x-]++*bold*++"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_eq!(children.len(), 1);
        assert_styled(&children[0], StyleVariant::Strong, SpanForm::Constrained);

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert!(styled.roles.is_empty(), "the `x-` marker clears the role");
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn an_x_dash_marker_with_a_leading_role_keeps_the_role() {
        // `[method x-]+save()+`: the trailing ` x-` is stripped, leaving
        // `method` as the surviving attrlist body. `styled.roles` (from
        // `Attrlist::roles`) does not itself capture a bare first positional
        // attribute like `method` – the renderer's own
        // `render_quoted_substitution` treats it as a role via
        // `nth_attribute(1).block_style()`, using `styled.attrs` (kept in
        // full) rather than `styled.roles` – so this is asserted through the
        // fold, which is what the differential corpus also pins.
        let nodes = build_src(Span::new("[method x-]+save()+"));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.variant, StyleVariant::Code);
                assert!(styled.attrs.is_some(), "the attribute list is retained");
            }

            other => panic!("expected Styled, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            r#"<code class="method">save()</code>"#
        );
    }

    #[test]
    fn the_x_dash_marker_only_applies_to_the_double_plus_boundary() {
        // `[x-]+++text+++` and `[x-]$$text$$`: unlike `++`, these two
        // boundaries never switch to old-behavior, so `x-` is kept as an
        // ordinary (literal) role rather than triggering monospace/`Normal`
        // subs.
        for source in ["[x-]+++text+++", "[x-]$$text$$"] {
            let nodes = build_src(Span::new(source));

            match &nodes[0] {
                InlineNode::Styled(styled) => {
                    assert_eq!(styled.variant, StyleVariant::Unquoted, "for {source:?}");
                }

                other => panic!("expected Styled for {source:?}, got {other:?}"),
            }

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                r#"<span class="x-">text</span>"#,
                "for {source:?}"
            );
        }
    }

    #[test]
    fn the_backtick_bare_form_is_always_monospace_under_verbatim_subs() {
        // `` [x-]`just *mono*` ``: the backtick form's attrlist is always
        // `x-`-eligible (the regex itself requires it), but its format mark
        // keeps `subs` at `Verbatim` regardless – `*mono*` stays literal,
        // unlike the plus form's `Normal`-subs old-behavior case.
        let nodes = build_src(Span::new("[x-]`just *mono*`"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_eq!(children.len(), 1);
        assert_raw(&children[0], "just *mono*");
    }

    #[test]
    fn the_plus_bare_form_without_x_dash_is_an_unquoted_span() {
        // `[.role]+text+`: an ordinary (non-`x-`) attrlist on the plus bare
        // form behaves like the delimited `++`/`$$` boundaries – `Unquoted`
        // under `Verbatim` subs.
        let nodes = build_src(Span::new("[.role]+text+"));

        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw(&children[0], "text");
    }

    #[test]
    fn an_escaped_attrlist_bracket_is_a_documented_divergence() {
        // `\[attrs]++text++` unescapes to a literal `[attrs]` prefix *and*
        // still recognizes the delimited text as an ordinary (non-attrlisted)
        // passthrough – a kept-literal-prefix-with-one-dropped-char, plus a
        // node for the remainder, a shape neither `MacroMatchKind` variant
        // expresses. Left fully unrecognized here.
        let source = r"\[attrs]++text++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes
                .iter()
                .all(|n| !matches!(n, InlineNode::Raw { .. } | InlineNode::Styled(_))),
            "an escaped attrlist bracket must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_eq!(folded, source);
        assert_ne!(folded, golden);
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_attrlisted_form_is_a_documented_divergence() {
        // The string pipeline's own `InlinePassReplacer` retries around a
        // match immediately preceded by `\`, `:`, or `;` (no lookbehind in
        // Rust's regex engine) – this increment does not reproduce the
        // retry, so such a match is simply left unrecognized.
        let source = "index:[attrs]+text+";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "a prohibited-prefix match must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(folded, source);
    }

    #[test]
    fn a_bare_attrlisted_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // Exercises `find_bare_attrlisted_matches`'s own `range_is_verbatim`
        // guard directly – the second pass's counterpart to
        // `a_match_whose_content_crosses_an_already_built_node_is_deferred`.
        // Reconstructed as flat text this level would read `[attrs]+x+`, but
        // the single-character body sits on an already-built (opaque)
        // `Styled` node rather than verbatim text, so the candidate match –
        // whose `full` range still spans it – is left unrecognized.
        let source = Span::new("[attrs]+x+");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("[attrs]+"),
                location: source.slice(0..8),
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location: source.slice(8..9),
            }),
            InlineNode::Text {
                value: CowStr::from("+"),
                location: source.slice(9..10),
            },
        ];

        let result = apply_passthroughs(nodes.clone(), source, &Parser::default());

        assert_eq!(
            result, nodes,
            "a non-verbatim bare-attrlisted match must be left unrecognized"
        );
    }

    #[test]
    fn a_bare_plus_attrlisted_delimiter_escape_stays_literal() {
        // `[attrs]\+text+`: honors the escape of the formatting mark – one
        // backslash drops, the rest (attrlist brackets included) stays
        // literal, mirroring `InlinePassReplacer`'s own `escape_count > 0`
        // branch. Unlike the delimited form's own escape
        // (`[.role]\++text++`), there is no further pass to re-scan the
        // residue here – this second pass already is the last one – so the
        // result is simple parity with the golden string pipeline.
        let source = r"[attrs]\+text+";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped bare-attrlisted delimiter must not build a Styled node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "[attrs]+text+");
        assert_eq!(folded, golden_passthroughs(source));
    }

    #[test]
    fn a_pass_macro_with_an_explicit_subs_list_is_a_documented_divergence() {
        // `pass:c[…]` names an explicit substitution list, which would need a
        // richer subtree than a single `Raw` leaf can hold (the same reason a
        // footnote's content is structured children rather than a literal
        // value) – deferred to a later increment.
        let source = "pass:c[<b>]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "a pass: macro with an explicit subs list must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
    }

    #[test]
    fn a_bare_unconstrained_passthrough_is_a_documented_divergence() {
        // The bare `+text+` form is matched by `INLINE_PASS`, not
        // `INLINE_PASS_MACRO` – its "must not follow a word" boundary needs a
        // lookbehind Rust's regex engine cannot express, which the string
        // replacer works around with a retry loop this increment does not
        // reproduce. Left unrecognized: the plus signs stay literal.
        let source = "a +text+ b";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "a bare unconstrained passthrough must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(folded, "a +text+ b");
    }
}
