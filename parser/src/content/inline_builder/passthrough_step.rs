//! The passthrough-extraction substitution step.

use super::{
    macros::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level},
    quotes::{Piece, attributes_of, build_match_string, source_slice},
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
/// for [`apply_special_characters`](super::special_chars::apply_special_characters)
/// and the later steps to refine.
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
/// Running as a genuine second pass has one consequence worth calling out: a
/// bare-attrlisted body whose content the *first* pass already recognizes on
/// its own – an embedded `pass:[…]`, or a `+++…+++`/`++…++`/`$$…$$` delimited
/// passthrough – is left **unrecognized** rather than wrapped, because the
/// candidate match's body then spans the already-built (opaque) node the
/// first pass left behind (the same `range_is_verbatim` boundary every macro
/// family in this module documents). The *delimited* attrlisted forms do not
/// share this gap: `INLINE_PASS_MACRO`'s own attrlist-prefixed alternative
/// matches the *whole* construct (attrlist, delimiters, and body) as one
/// leftmost match, so an embedded `pass:[…]` inside, say, `[x-]++pass:[<b>]++`
/// is captured as part of that one match's body – never independently matched
/// first – and (per the `x-` marker below) extracted correctly by the
/// recursive `Normal`-order substitution its body then runs through.
///
/// The **bare unconstrained form** (`+text+`, no attribute list) folds
/// through a plain [`Raw`](InlineNode::Raw) leaf – like the double-plus/
/// double-dollar forms, an absent attrlist means no stored `type_`, so
/// `PassthroughRestoreReplacer` never wraps the restored text in a rendered
/// span. Unlike the two attribute-list-prefixed bare forms above (matched via
/// `\b{start-half}`, which does not by itself exclude a `\`/`:`/`;` prefix),
/// this form's own pattern already excludes that "prohibited prefix" – and the
/// word-boundary rule the doc comment on [`find_bare_attrlisted_matches`]
/// explains – directly in its *consuming* boundary group (`[^\w;:\\]`), so no
/// runtime retry is needed: a match simply cannot start where the pattern's
/// own character class would reject it. The boundary character it does
/// consume (present unless the match sits at the very start of the level) is
/// not part of the construct – it is kept as literal text before the node,
/// reusing the same kept-prefix [`MacroMatch`] sub-range the auto-link
/// increment introduced.
///
/// A **`pass:` macro carrying an explicit substitution list**
/// (`pass:c,q[…]`) folds through [`build_pass_macro_subs_value`], the same
/// [`Raw`](InlineNode::Raw) shape [`build_passthrough_node`] gives every
/// other `pass:`/delimiter form: `text` runs through the real, string-based
/// substitution pipeline under the resolved
/// [`SubstitutionGroup::Custom`] list (mirroring
/// `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call), producing
/// an already-final HTML string that becomes the leaf's `value` verbatim.
/// See [`build_pass_macro_subs_value`]'s own doc comment for why a `Raw`
/// leaf – not a richer node subtree built from this module's own
/// transducers – is the shape this increment needs: the resolved list can
/// name any of the six steps in any order, and only an opaque leaf is immune
/// to [`build`](super::build)'s own later steps reprocessing (or, for a step
/// the list omits, wrongly applying to) that same content a second time.
/// Inline STEM (`stem:[…]`, `asciimath:[…]`,
/// `latexmath:[…]`) is an implicit
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
/// An **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops
/// its backslash and keeps the delimited text literal at the pass-macro
/// level, but – now that the bare unconstrained form is recognized too – the
/// *second* pass ([`INLINE_PASS`]) legitimately re-scans that same de-escaped
/// text and consumes its leading `+++`/`++` as a bare passthrough wrapping a
/// shorter run (`+text` / `text`, one `+` left over as trailing literal
/// text), exactly as the string pipeline's own second regex pass does over
/// its own once-substituted text: parity, not a divergence
/// (`an_escaped_triple_plus_reveals_a_nested_bare_passthrough`,
/// `an_escaped_double_plus_reveals_a_nested_bare_passthrough`). An escaped
/// `$$…$$` or `pass:[…]` has no such residue and stays parity for the same
/// reason regardless, since [`INLINE_PASS`] never matches `$$` or `pass:`
/// syntax. An escaped attribute-list-prefixed *delimiter* (`[attrs]\++text++`)
/// is parity for the same reason: the delimiter's own `Unescape` leaves
/// literal, unopaqued text behind, so the bare-form second pass legitimately
/// re-recognizes it.
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

/// Finds every passthrough at this level, skipping the one form
/// [`apply_passthroughs`] still documents as deferred: an attribute-list-
/// prefixed match whose *bracket* is escaped (`\[attrs]++text++`). A
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

/// Finds every bare passthrough at this level – the two attribute-list-
/// prefixed [`INLINE_PASS`] options (`` [x-]`text` `` and `[attrs]+text+`)
/// and the bare unconstrained form with no attribute list (`+text+`, built by
/// [`build_bare_unconstrained_match`]).
///
/// The two attribute-list-prefixed options also need a lookbehind the string
/// replacer works around with a retry loop (`InlinePassReplacer`'s
/// "prohibited prefix" check): a match immediately preceded by `\`, `:`, or
/// `;` is not really a passthrough. This increment does not reproduce that
/// retry for *them*; instead it simply leaves such a match unrecognized, a
/// documented divergence pinned by a test. The bare unconstrained form needs
/// no such handling – see [`build_bare_unconstrained_match`].
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

        if !is_backtick && !is_plus_attrlisted {
            // Option 3: the bare unconstrained form (no attribute list).
            if let Some(m) = build_bare_unconstrained_match(&caps, &full, pieces, root, parser) {
                matches.push(m);
            }

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

/// Builds one [`MacroMatch`] for a bare unconstrained [`INLINE_PASS`] match
/// (`+text+`, option 3 – no attribute list): a plain [`Raw`](InlineNode::Raw)
/// leaf, computed through the real substitution pipeline under
/// [`SubstitutionGroup::Verbatim`] exactly like the double-plus/double-dollar
/// forms (an absent attrlist means no stored `type_`, so
/// `PassthroughRestoreReplacer` never wraps the restored text in a rendered
/// span – unlike [`build_bare_attrlisted_passthrough_node`]'s `Styled` result).
///
/// Group 8 (the body) always participates whenever this alternative matches at
/// all, and the pattern's own leading/trailing `+` delimiters sit exactly one
/// byte to either side of it, so their offsets are derived rather than
/// captured separately. The optional boundary character the pattern consumes
/// ahead of the leading `+` (Group 6 – absent only when the match sits at the
/// very start of the level, via the pattern's `^` alternative) is not part of
/// the construct itself; it is kept as literal text before the node via the
/// same kept-prefix [`MacroMatchKind::Node`] sub-range the auto-link increment
/// introduced for a bare URL's own boundary prefix.
///
/// An escaped mark (`\+text+`, Group 7) drops the single backslash and keeps
/// the rest of the match – the boundary character included – as literal text,
/// with nothing left to re-scan it afterward (this is already the last pass),
/// so it is plain parity rather than a divergence.
///
/// Returns `None` for a non-verbatim match (crossing an escaped special or a
/// rendered span), left unrecognized exactly as every other macro family in
/// this module defers one.
fn build_bare_unconstrained_match<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    if !range_is_verbatim(pieces, full) {
        return None;
    }

    // The body (option 3's mandatory capture whenever this alternative
    // matches) is always preceded immediately by the construct's opening `+`.
    #[allow(clippy::unwrap_used)]
    let body_m = caps.get(8).unwrap();
    let delim_start = body_m.start() - 1;

    if let Some(escape) = caps.get(7) {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: escape.start(),
            },
            full: full.clone(),
        });
    }

    let consumed = delim_start..full.end;
    let location = source_slice(pieces, consumed.clone(), root);
    let body_span = source_slice(pieces, body_m.start()..body_m.end(), root);
    let value = passthrough_text(body_span.data(), &SubstitutionGroup::Verbatim, parser);

    Some(MacroMatch {
        kind: MacroMatchKind::Node {
            consumed,
            node: Box::new(InlineNode::Raw {
                value: CowStr::from(value),
                location,
            }),
        },
        full: full.clone(),
    })
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

    // The `pass:` macro (no delimiters). With an **explicit substitution
    // list** (`pass:c,q[…]`, group 14), the body is rendered through the
    // real substitution pipeline under the resolved `SubstitutionGroup::
    // Custom` list – see [`build_pass_macro_subs_value`] for why this (and
    // not a richer node subtree) is the safe shape for this increment.
    // Without one (the bare `pass:[…]` form), `SubstitutionGroup::None`
    // applies nothing. Either way, an escaped closing bracket (`\]`)
    // unescapes first, mirroring the string replacer's
    // `text.replace("\\]", "]")` – the same treatment every other macro
    // family's bracket content gets.
    #[allow(clippy::unwrap_used)]
    let m = caps.get(15).unwrap();
    let content = source_slice(pieces, m.start()..m.end(), root);
    let raw = content.data();
    let unescaped = raw.contains("\\]").then(|| raw.replace("\\]", "]"));

    let value = if let Some(subs_list) = caps.get(14) {
        let text = unescaped.as_deref().unwrap_or(raw);
        CowStr::from(build_pass_macro_subs_value(
            text,
            subs_list.as_str(),
            parser,
        ))
    } else if let Some(unescaped) = unescaped {
        CowStr::from(unescaped)
    } else {
        CowStr::from(raw)
    };

    InlineNode::Raw { value, location }
}

/// Computes the rendered `value` for a `pass:` macro carrying an **explicit
/// substitution list** (`pass:c,q[…]`) – the one deferred form 5a documented
/// that a bare `SubstitutionGroup::None`/`::Verbatim` treatment cannot cover,
/// since the list can name *any* of the six named steps, in any order and
/// combination the author writes.
///
/// A naive extension would thread `text` through this module's own node
/// transducers under the resolved step list, the way the legacy `x-`
/// compatibility marker's body does ([`apply_normal_subs`]). That shape does
/// not work here: [`build`](super::build) always runs its own fixed *normal*
/// order over the level this passthrough is embedded in, so any structural
/// node (`Styled`, `Ref`, …) this construct's own resolved subset produced
/// would be visited *again* by whichever of `build`'s six steps come after
/// this one – and unlike `Quotes` (whose delimiters are consumed, so a
/// second pass finds nothing left to match) or `SpecialCharacters` (whose
/// `CharRef` leaves are atomic), a macro's own display text is not
/// idempotent under a second pass: a `Ref{Link}`'s display children are
/// literal text that *looks* exactly like the source URL, so a second
/// `Macros` pass would recognize it all over again and nest a nested link
/// inside it. A resolved list omitting a step `build`'s own fixed order
/// still runs (e.g. `pass:q[<b>]`, which never asks for
/// `SpecialCharacters`) has the same problem in reverse: `build`'s own
/// unconditional `SpecialCharacters` step would escape content the author's
/// list deliberately left raw.
///
/// [`passthrough_text`] – already used for `++…++`/`$$…$$`/the bare
/// unconstrained form – sidesteps both failure modes: it renders `text`
/// through the **real, string-based** substitution pipeline
/// ([`SubstitutionGroup::apply`], the same call
/// `PassthroughRestoreReplacer` makes for a stored `Passthrough`), producing
/// an already-final HTML string, then this function's caller wraps it in a
/// single [`Raw`](InlineNode::Raw) leaf. A `Raw` leaf is *opaque* to every
/// later step in this module (never descended into, never re-matched –
/// design §4.2's passthrough-as-leaf convention), so it is immune to both
/// failure modes above: nothing in `build`'s own remaining steps can touch
/// it, whether or not the author's list included that step.
///
/// An unrecognized substitution name in the list (e.g. `pass:bogus[…]`) is
/// silently skipped – any recognized names are still honored – mirroring
/// [`SubstitutionGroup::from_custom_string`]/`InlinePassMacroReplacer`'s own
/// resolution. Unlike the string pipeline, this additive pass does not yet
/// raise the `InvalidSubstitutionTypeForPassthroughMacro` warning for it,
/// deferring that side effect to the cutover exactly as every other macro
/// family defers its own catalog/warning side effect (design §5.2 Phase 4
/// step 6), since it does not change the fold's output bytes.
fn build_pass_macro_subs_value(text: &str, subs_list: &str, parser: &Parser) -> String {
    let (subs, _invalid) = SubstitutionGroup::from_custom_string(None, subs_list);
    passthrough_text(text, &subs, parser)
}

/// Builds one [`Styled`] node from a verbatim, unescaped, attribute-listed
/// `INLINE_PASS_MACRO` match (`[attrs]+++text+++`, `[attrs]++text++`,
/// `[attrs]$$text$$`) – the delimited half of the attribute-list-prefixed
/// forms this increment recognizes (see
/// [`build_bare_attrlisted_passthrough_node`] for the bare half). Folds through
/// the same `render_quoted_substitution` `PassthroughRestoreReplacer` calls
/// when its stored passthrough carries a `type_`/`attrlist`, so the output is
/// byte-for-byte identical.
///
/// Only the `++` boundary can trigger the legacy `x-` compatibility marker
/// (`handle_quoted_text`'s `old_behavior`, see
/// [`split_old_behavior_attrlist`]); `+++`/`$$` always keep the attrlist as
/// written and never switch to the `Normal` substitution group.
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

/// Runs `text` through the full [`SubstitutionGroup::Normal`] pipeline and
/// returns the resulting node subtree, for the legacy `x-` compatibility
/// marker's `Normal`-group passthrough body (see
/// [`split_old_behavior_attrlist`]).
///
/// This mirrors `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call
/// for that case. That call is *not* just the six named steps
/// (`SpecialCharacters`, `Quotes`, `AttributeReferences`,
/// `CharacterReplacements`, `Macros`, `PostReplacement`): `SubstitutionGroup`'s
/// `run_pipeline` extracts passthroughs (and, as part of that same extraction,
/// inline STEM) *before* running a group's steps whenever the group's step
/// list includes `Macros` – which `Normal`'s does – so a passthrough or STEM
/// macro nested in an `x-` body (`[x-]++pass:[<b>]++`) is itself extracted
/// and restored, not left for `Macros` to walk over as plain text.
/// [`build`](super::build)
/// already threads a span through exactly that full sequence – passthroughs,
/// STEM, then the six steps, footnotes included (the string pipeline's
/// `Macros` step recognizes footnote macros too, so `Normal`'s semantics
/// cover them; `build` only splits that recognition into its own step for
/// numbering-order reasons, design §5.2 step 4b(ii) part 4c) – so this
/// delegates to it directly rather than re-deriving a subset.
fn apply_normal_subs<'src>(text: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
    super::build(text, parser)
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
        apply_pass_macro_level, apply_passthroughs,
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
        // In practice `apply_pass_macro_level` only ever runs on the pristine
        // whole-source seed (it is `build`'s first step, ahead of every node
        // that could make a range non-verbatim), so `range_is_verbatim`'s
        // false branch is defensive – kept for the same reason every other
        // macro family keeps the check. Exercise it directly, feeding a
        // hand-built level whose triple-plus content spans an already-built
        // `Styled` node, to document the intended fallback: the whole match
        // is left unrecognized rather than mis-sliced.
        //
        // This calls `apply_pass_macro_level` (not the top-level
        // `apply_passthroughs`) to isolate this guard from the bare
        // unconstrained pass that runs after it: each leftover `+++` run is,
        // on its own, itself a valid (non-crossing) bare passthrough – see
        // `a_bare_unconstrained_match_whose_content_crosses_an_already_built_node_is_deferred`
        // for that guard exercised directly on the second pass instead.
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
        let result = apply_pass_macro_level(nodes.clone(), root, &Parser::default());

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
    fn an_escaped_triple_plus_reveals_a_nested_bare_passthrough() {
        // The pass-macro-level pass drops the single backslash and keeps
        // `+++text+++` literal there, mirroring every other family's
        // `\image:…` escape handling. Now that the bare unconstrained form is
        // recognized, the *second* pass (`INLINE_PASS`) legitimately re-scans
        // that same de-escaped text and consumes its leading `+++` as a bare
        // passthrough wrapping a single `+` (the outer `+` is the delimiter,
        // the middle `+` is the body), leaving the third `+` as literal text
        // in front of `text+++` – exactly what the string pipeline's own
        // second regex pass does over its own once-substituted text, so this
        // is parity, not a divergence.
        let source = r"\+++text+++";
        let nodes = build_src(Span::new(source));

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "++text++");
        assert_eq!(folded, golden_passthroughs(source));
    }

    #[test]
    fn an_escaped_double_plus_reveals_a_nested_bare_passthrough() {
        // Same mechanism as `an_escaped_triple_plus_reveals_a_nested_bare_passthrough`,
        // one delimiter layer down: de-escaping `\++text++` to `++text++`
        // leaves a leading `++` the bare-form pass now legitimately
        // re-consumes as a bare passthrough wrapping `+text`, leaving a
        // single trailing `+` as literal text.
        let source = r"\++text++";
        let nodes = build_src(Span::new(source));

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "+text+");
        assert_eq!(folded, golden_passthroughs(source));
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
            // The `pass:` macro with an explicit substitution list: a single
            // step, several steps (applied in the order given, not the
            // normal order), an unrecognized name skipped alongside a
            // recognized one, an empty resolved body, and an escape.
            "pass:c[<b>]",
            "pass:q[*bold*]",
            "pass:c,q[<b> *bold*]",
            "pass:q,c[<b> *bold*]",
            "pass:m[https://example.org]",
            "pass:a[{missing-attr}]",
            "pass:bogus[<b>]",
            "pass:c,bogus[<b>]",
            "pass:c[]",
            r"\pass:c[<b>]",
            // Multiple passthroughs, and passthroughs beside ordinary quoted
            // text (proving the surrounding text is still substituted
            // normally).
            "a +++x+++ and a ++y++ and a $$z$$ and a pass:[w]",
            "*bold* +++<raw/>+++ *more bold*",
            // Escapes. The `+++`/`++` forms are pinned separately (see
            // `an_escaped_triple_plus_reveals_a_nested_bare_passthrough` and
            // its double-plus counterpart), since de-escaping them leaves a
            // residue the bare `+text+` pass now legitimately re-consumes.
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
            // The `x-` marker's `Normal`-order body extracts a nested
            // passthrough/STEM/footnote/macro rather than walking over it as
            // plain text (`SubstitutionGroup::Normal`'s `run_pipeline`
            // extracts passthroughs, and thus STEM, ahead of its step list
            // whenever `Macros` is in scope, which it is for `Normal`).
            "[x-]++pass:[<b>]++",
            "[x-]++stem:[x^2]++",
            "[x-]++footnote:[note text]++",
            "[x-]++image:x.png[X] and pass:[<i>]++",
            // The bare unconstrained form (`INLINE_PASS`'s option 3, no
            // attribute list): at the start of the flow, mid-flow (keeping
            // its boundary prefix as literal text), and escaping specials
            // only (quotes never run over the body).
            "+text+",
            "a +text+ b",
            "+<b>*not quotes*</b>+",
            "multiple +one+ and +two+ passthroughs",
            // Escapes: the mark drops its backslash and stays literal, with
            // the boundary prefix preserved.
            r"see \+text+ end",
            r"\+text+",
            // A prefix the pattern's own consuming boundary group excludes
            // (`\`, `:`, `;`) leaves the match unrecognized by both
            // pipelines, so the source stays entirely literal.
            r"a\+text+ b",
            "a:+text+ b",
            "a;+text+ b",
            // Beside ordinary flow and next to other constructs.
            "*bold* +text+ _em_",
            "a copyright (C) then +text+",
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
    fn the_x_dash_marker_normal_subs_extracts_a_nested_passthrough() {
        // `[x-]++pass:[<b>]++`: `SubstitutionGroup::Normal`'s own
        // `run_pipeline` extracts passthroughs (and STEM) *before* running its
        // step list whenever `Macros` is in scope – which it is for `Normal` –
        // so a `pass:[…]` nested in the `x-` body is itself extracted and
        // restored by `apply_normal_subs`'s delegation to `build`, rather than
        // being left for `apply_macros` (which does not recognize `pass:` at
        // all) to walk over as plain text.
        let nodes = build_src(Span::new("[x-]++pass:[<b>]++"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_raw(&children[0], "<b>");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs("[x-]++pass:[<b>]++")
        );
    }

    #[test]
    fn the_x_dash_marker_normal_subs_extracts_a_nested_stem_macro() {
        // `[x-]++stem:[x^2]++`: STEM is extracted in the same pass as
        // passthroughs (`Passthroughs::extract_from` runs both), so a nested
        // STEM macro is likewise recognized rather than left as literal text.
        let nodes = build_src(Span::new("[x-]++stem:[x^2]++"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_eq!(children.len(), 1);
        assert!(
            matches!(children[0], InlineNode::Stem(_)),
            "expected a nested Stem node, got {:?}",
            children[0]
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs("[x-]++stem:[x^2]++")
        );
    }

    #[test]
    fn the_x_dash_marker_normal_subs_numbers_a_nested_footnote() {
        // `[x-]++footnote:[…]++`: the string pipeline's `Macros` step
        // recognizes footnote macros too, so `Normal`'s semantics cover them;
        // `build` splits that recognition into its own `apply_footnotes` step
        // (for numbering-order reasons), which delegating to `build` picks up
        // for free.
        let nodes = build_src(Span::new("[x-]++footnote:[note text]++"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_eq!(children.len(), 1);
        assert!(
            matches!(children[0], InlineNode::Footnote(_)),
            "expected a nested Footnote node, got {:?}",
            children[0]
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs("[x-]++footnote:[note text]++")
        );
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
    fn a_bare_attrlisted_form_wrapping_an_embedded_macro_pass_is_a_documented_divergence() {
        // `[method x-]+pass:[<b>]+`: the *macro* pass (`apply_pass_macro_level`,
        // which runs first) recognizes the embedded `pass:[<b>]` on its own –
        // `INLINE_PASS_MACRO`'s `pass:` alternative matches that substring
        // regardless of surrounding context – *before* the bare-form second
        // pass gets a chance to see `[method x-]+…+` as one attrlisted
        // construct. The candidate bare-form match's body then spans the
        // already-built (opaque) node the macro pass left behind, so
        // `range_is_verbatim` defers it – the real-world trigger for
        // `a_bare_attrlisted_match_whose_content_crosses_an_already_built_node_is_deferred`'s
        // guard, rather than a hand-built level.
        //
        // The string pipeline instead reconciles this via a recursive
        // sentinel-restoration step (`PassthroughRestoreReplacer` re-resolves
        // a leftover placeholder against the *outer* passthrough list when a
        // nested `Normal`-group substitution doesn't consume it) that has no
        // counterpart in this tree-based, single-pass builder.
        let source = "[method x-]+pass:[<b>]+";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "a bare-attrlisted match crossing an embedded macro must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(golden, r#"<code class="method"><b></code>"#);
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
    fn a_pass_macro_with_a_special_characters_subs_list_is_a_raw_node() {
        // `pass:c[…]` resolves to `Custom([SpecialCharacters])`: the body is
        // rendered through the real pipeline under just that one step (see
        // `build_pass_macro_subs_value`), so `<`/`>` are already escaped in
        // the leaf's `value` – a single opaque `Raw` node, not `CharRef`
        // leaves this builder's own `SpecialCharacters` transducer would
        // produce.
        let source = "pass:c[<b>]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt;");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_with_a_quotes_subs_list_renders_the_markup() {
        // `pass:q[…]` resolves to `Custom([Quotes])`: rendered through the
        // real pipeline, so the leaf's `value` already carries the
        // `<strong>` markup `Quotes` produced – unescaped, since `Raw`'s
        // fold emits `value` verbatim.
        let source = "pass:q[*bold*]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "<strong>bold</strong>");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_with_a_macros_subs_list_renders_a_nested_macro() {
        // `pass:m[…]` resolves to `Custom([Macros])`: rendered through the
        // real pipeline (which itself extracts passthroughs/STEM ahead of
        // `Macros`, `run_pipeline`'s own gate), so the leaf's `value` already
        // carries the rendered `<a href="…">` markup.
        let source = "pass:m[https://example.org]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(
            &nodes[0],
            r#"<a href="https://example.org" class="bare">https://example.org</a>"#,
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_with_multiple_subs_applies_them_in_the_order_given() {
        // `pass:q,c[…]` resolves to `Custom([Quotes, SpecialCharacters])` –
        // the order the author wrote, not the *normal* effective order
        // (which always runs `SpecialCharacters` first). `Quotes` runs
        // first here, wrapping `*bold*` in a literal `<strong>…</strong>`;
        // `SpecialCharacters` then runs *second* and escapes every `<`/`>`
        // it finds – tags included, since it has no way to tell them apart
        // from `<b>`'s own literal angle brackets. This is the documented
        // gotcha of naming `specialcharacters` after a step that emits
        // markup, reproduced byte-for-byte from the real pipeline.
        let nodes = build_src(Span::new("pass:q,c[<b> *bold*]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt; &lt;strong&gt;bold&lt;/strong&gt;");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs("pass:q,c[<b> *bold*]")
        );
    }

    #[test]
    fn a_pass_macro_with_an_unrecognized_subs_name_skips_it() {
        // An unrecognized name resolves to zero steps – rather than
        // invalidating the whole list – mirroring
        // `SubstitutionGroup::from_custom_string`/`InlinePassMacroReplacer`'s
        // own "skip and keep going" resolution. With no steps at all the
        // rendered `value` is the content completely untouched (not even
        // special characters are escaped).
        let source = "pass:bogus[<b>]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "<b>");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_recognized_name_beside_an_unrecognized_one_is_still_honored() {
        // `pass:c,bogus[…]` resolves to the same `Custom([SpecialCharacters])`
        // as `pass:c[…]` alone – the unrecognized name is skipped, not fatal
        // to the rest of the list.
        let nodes = build_src(Span::new("pass:c,bogus[<b>]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt;");
    }

    #[test]
    fn an_escaped_pass_macro_with_a_subs_list_stays_literal() {
        // `\pass:c[…]` drops the single backslash and reconstructs the whole
        // `pass:c[…]` text literally, mirroring `InlinePassMacroReplacer`'s
        // own `caps.get(13)` branch (which re-emits `pass:`, the subs list,
        // and the bracketed content exactly as written).
        let source = r"\pass:c[<b>]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not apply its subs list: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_subs_list_unescapes_an_escaped_closing_bracket() {
        // `pass:c[a\]b]`: the same `text.replace("\\]", "]")` unescape every
        // other `pass:[…]` bracket content gets, applied before the resolved
        // list renders it.
        let nodes = build_src(Span::new(r"pass:c[a\]b]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "a]b");
    }

    #[test]
    fn a_bare_unconstrained_passthrough_is_a_raw_node() {
        // `+text+` with no attribute list folds through a plain `Raw` leaf –
        // like the double-plus/double-dollar forms, an absent attrlist means
        // no stored `type_`, so the restore never wraps the text in a
        // rendered span.
        let nodes = build_src(Span::new("a +text+ b"));

        assert_eq!(nodes.len(), 3);
        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "a text b");

        match &nodes[1] {
            InlineNode::Raw { value, location } => {
                assert_eq!(value.as_ref(), "text");
                assert_eq!(location.data(), "+text+");
            }

            other => panic!("expected Raw, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs("a +text+ b")
        );
    }

    #[test]
    fn a_bare_unconstrained_passthrough_escapes_specials_only() {
        // `SubstitutionGroup::Verbatim` applies only special characters, the
        // same treatment the delimited `++…++`/`$$…$$` forms get.
        let nodes = build_src(Span::new("+<b>*not quotes*</b>+"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt;*not quotes*&lt;/b&gt;");
    }

    #[test]
    fn a_bare_unconstrained_passthrough_at_the_very_start_has_no_kept_prefix() {
        // At the very start of the level there is no boundary character to
        // consume (the pattern's `^` alternative), so the node's location
        // covers the construct alone.
        let nodes = build_src(Span::new("+text+ after"));

        assert_eq!(nodes.len(), 2);
        assert_raw(&nodes[0], "text");
        assert_eq!(nodes[0].span().data(), "+text+");
    }

    #[test]
    fn an_escaped_bare_unconstrained_passthrough_stays_literal() {
        // `\+text+` drops the single backslash and keeps `+text+` literal –
        // no `Raw` node – with the boundary prefix (here "see ") preserved.
        // Unlike the pass-macro level's own `+++`/`++` escapes, there is no
        // further pass to re-scan the residue here (this is already the last
        // pass), so this is plain parity, not a divergence.
        let source = r"see \+text+ end";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_unconstrained_form_needs_no_retry() {
        // Unlike the two attribute-list-prefixed bare forms (which need a
        // documented divergence for this – see
        // `a_prohibited_prefix_before_a_bare_attrlisted_form_is_a_documented_divergence`),
        // the bare unconstrained form's own pattern already excludes a `\`,
        // `:`, or `;` prefix in its consuming boundary group, so this is
        // parity, not a divergence: the passthrough is correctly left
        // unrecognized *and* the golden string pipeline agrees.
        for source in [r"a\+text+ b", "a:+text+ b", "a;+text+ b"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
                "a prohibited-prefix match must be left unrecognized for {source:?}: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_passthroughs(source),
                "for {source:?}"
            );
        }
    }

    #[test]
    fn a_bare_unconstrained_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // A candidate bare-unconstrained match whose body spans an
        // already-built (opaque) node – here a `Styled` span from a hand-built
        // level, standing in for what an earlier pass-macro-level match would
        // leave behind – is left unrecognized rather than mis-sliced, the same
        // guard every other macro family in this module keeps.
        let source = Span::new("+x+");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("+"),
                location: source.slice(0..1),
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location: source.slice(1..2),
            }),
            InlineNode::Text {
                value: CowStr::from("+"),
                location: source.slice(2..3),
            },
        ];

        let result = apply_passthroughs(nodes.clone(), source, &Parser::default());

        assert_eq!(
            result, nodes,
            "a non-verbatim bare-unconstrained match must be left unrecognized"
        );
    }
}
