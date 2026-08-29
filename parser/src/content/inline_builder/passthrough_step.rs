//! The passthrough-extraction substitution step.

use super::{
    macros::{
        MacroMatch, MacroMatchKind,
        image::{node_is_restorable, range_is_restorable, range_is_verbatim, restorable_body},
        rebuild_macro_level,
    },
    quotes::{Piece, attributes_of, build_match_string, source_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span,
    content::{INLINE_PASS, INLINE_PASS_MACRO, SubstitutionGroup},
    inlines::{InlineNode, PassthroughWrapper, RawForm, RawOrigin, SpanForm, StyleVariant, Styled},
    strings::CowStr,
};

/// The passthrough-extraction step, as a node transducer: replaces each
/// recognized passthrough with a [`Raw`](InlineNode::Raw) leaf and leaves
/// everything else as the whole-source seed [`Text`](InlineNode::Text) node,
/// for [`apply_special_characters`](super::special_chars::apply_special_characters)
/// and the later steps to refine.
///
/// This is the **first** step [`build`](super::build) runs — mirroring
/// `Passthroughs::extract_from`,
/// which the string pipeline runs *before* its own step loop — so a
/// passthrough's content is never touched by specialcharacters, quotes,
/// replacements, or macros: it is a leaf, and every later step's
/// [`build_match_string`] already treats a node it does not specifically
/// handle (an already-built [`Styled`] span, and now a
/// [`Raw`](InlineNode::Raw) leaf) as a
/// single opaque placeholder.
///
/// It reuses the string pipeline's *exact* recognition —
/// [`INLINE_PASS_MACRO`] is now shared `pub(crate)` — so only the recognition
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
/// [`InlineRenderer`](crate::parser::InlineRenderer)'s
/// escaping is honored, exactly as it would be for the string pipeline's own
/// restore step — the cost is an owned [`Raw`](InlineNode::Raw) value rather
/// than a `'src` borrow, since the pipeline's output is not guaranteed to
/// coincide with the source.
///
/// An **attribute-list-prefixed** passthrough (`[quotes]++text++`,
/// `` [x-]`text` ``, `[attrs]+text+`) folds through a [`Styled`] node instead:
/// [`build_attrlisted_passthrough_node`] and
/// [`build_bare_attrlisted_passthrough_node`] parse the attrlist the same way
/// an attributed quote does ([`attributes_of`]) and wrap the body — itself a
/// `Raw` leaf under `SubstitutionGroup::None`/`Verbatim`, unless the legacy
/// `x-` compatibility marker switches it to a full `Normal`-order subtree
/// ([`apply_normal_subs`]) — in `Code` (monospace) or `Unquoted`, mirroring
/// `PassthroughRestoreReplacer`'s own `render_styled` call for a
/// stored passthrough whose `type_` is `Some`. This runs as a **second pass**
/// ([`apply_bare_attrlisted_pass_level`]) after the delimited forms above,
/// mirroring `Passthroughs::extract_from`'s own order (`INLINE_PASS_MACRO`
/// before [`INLINE_PASS`]).
///
/// Running as a genuine second pass has one consequence worth calling out: a
/// bare-attrlisted body whose content the *first* pass already recognizes on
/// its own — an embedded `pass:[…]`, or a `+++…+++`/`++…++`/`$$…$$` delimited
/// passthrough — is left **unrecognized** rather than wrapped, because the
/// candidate match's body then spans the already-built (opaque) node the
/// first pass left behind (the same `range_is_verbatim` boundary every macro
/// family in this module documents). The *delimited* attrlisted forms do not
/// share this gap: `INLINE_PASS_MACRO`'s own attrlist-prefixed alternative
/// matches the *whole* construct (attrlist, delimiters, and body) as one
/// leftmost match, so an embedded `pass:[…]` inside, say, `[x-]++pass:[<b>]++`
/// is captured as part of that one match's body — never independently matched
/// first — and (per the `x-` marker below) extracted correctly by the
/// recursive `Normal`-order substitution its body then runs through.
///
/// The **bare unconstrained form** (`+text+`, no attribute list) folds
/// through a plain [`Raw`](InlineNode::Raw) leaf — like the double-plus/
/// double-dollar forms, an absent attrlist means no stored `type_`, so
/// `PassthroughRestoreReplacer` never wraps the restored text in a rendered
/// span. Unlike the two attribute-list-prefixed bare forms above (matched via
/// `\b{start-half}`, which does not by itself exclude a `\`/`:`/`;` prefix and
/// so needs the retry below), this form's own pattern already excludes that
/// "prohibited prefix" — and the word-boundary rule the doc comment on
/// [`collect_bare_pass_matches`] explains — directly in its *consuming*
/// boundary group (`[^\w;:\\]`), so no retry is needed for it: a match simply
/// cannot start where the pattern's own character class would reject it. The
/// boundary character it does consume (present unless the match sits at the
/// very start of the level) is not part of the construct — it is kept as
/// literal text before the node, reusing the same kept-prefix [`MacroMatch`]
/// sub-range the auto-link increment introduced. Because it runs in the
/// *second* pass, its body may enclose a construct the first pass already
/// replaced (`+a $$b$$ c+`, `` +you feel pass:q[`mono`].+ ``): the verbatim
/// substitution runs over the placeholder as the string pipeline runs it over
/// its own sentinel, and the inner body is spliced in **after** — see
/// [`build_bare_unconstrained_match`]. The attribute-list-prefixed forms keep
/// their verbatim gate, so `[method x-]+pass:[<b>]+` stays deferred.
///
/// A **prohibited prefix** ahead of either attribute-list-prefixed bare form
/// (`index:[attrs]+text+`, `` \[x-]`text` ``) is answered the way the string
/// replacer answers it, for want of a lookbehind: the match's first character
/// — always its `[` — is written back verbatim and the *rest of that same
/// match* is scanned again ([`collect_bare_pass_matches`], recursively). The
/// second scan routinely recognizes a different, shorter construct the
/// bracket was hiding — for the plus form, the bare unconstrained form over
/// the same body, so the attribute list ends up a literal prefix and the body
/// an *ordinary* passthrough rather than a `Styled` span — and for the
/// backtick form it finds nothing, leaving the construct to the later quotes
/// step. Writing both of this pass's escapes at once
/// (`\[attrs]\++text++`) lands here as well: the delimiter escape wins the
/// first pass's branch, and the literal `++text++` it leaves behind sits
/// behind exactly the `\` this second pass declines.
///
/// A **`pass:` macro carrying an explicit substitution list**
/// (`pass:c,q[…]`) folds through the same [`Raw`](InlineNode::Raw) shape
/// [`build_passthrough_node`] gives every other `pass:`/delimiter form:
/// `text` runs through the real, string-based substitution pipeline under
/// the resolved [`SubstitutionGroup::Custom`] list (mirroring
/// `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call), producing
/// an already-final HTML string that becomes the leaf's `value` verbatim.
/// See [`build_passthrough_node`]'s own explicit-list arm for why a `Raw`
/// leaf — not a richer node subtree built from this module's own
/// transducers — is the shape this increment needs: the resolved list can
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
/// An **escaped bracket** (`\[attrs]++text++`) unescapes to a literal
/// `[attrs]` prefix *and* still recognizes the delimited text after it — as an
/// *ordinary* (non-attrlisted) passthrough, since `handle_quoted_text` drops
/// the attribute list on that branch rather than storing it. One match's
/// source doing two things is expressed as a **pair** of matches — an
/// [`Unescape`](MacroMatchKind::Unescape) over the bracket, then a
/// [`Node`](MacroMatchKind::Node) over the delimited remainder — which
/// [`rebuild_macro_level`] composes exactly as it composes any two adjacent
/// matches, so neither [`MacroMatchKind`] variant grows a new shape.
///
/// An **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops
/// its backslash and keeps the delimited text literal at the pass-macro
/// level, but — now that the bare unconstrained form is recognized too — the
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
/// [`InlineRenderer`](crate::parser::InlineRenderer):
/// crate::parser::InlineRenderer
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
    // string) is untouched by this second pass. A bare `+…+` may still
    // *enclose* one, which the string pipeline reads as ordinary body text
    // around its own sentinel and restores last; see
    // [`build_bare_unconstrained_match`] for how that order is reproduced.
    apply_bare_attrlisted_pass_level(nodes, root, parser)
}

/// The `INLINE_PASS_MACRO` pass: `+++…+++`, `++…++`, `$$…$$`, and `pass:[…]`,
/// with or without an attribute list ahead of the delimiters.
fn apply_pass_macro_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

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
/// attribute list (`+text+`) is deferred — see
/// [`find_bare_attrlisted_matches`].
fn apply_bare_attrlisted_pass_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // Cheap pre-filter mirroring `Passthroughs::extract_from`'s own guard for
    // `INLINE_PASS`.
    if !(s.contains('+') || s.contains("-]")) {
        return nodes;
    }

    let matches = find_bare_attrlisted_matches(&s, &nodes, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every passthrough at this level.
///
/// The two attribute-list-prefixed escapes take different shapes, mirroring
/// `handle_quoted_text`'s own two branches. A *delimiter* escape
/// (`[attrs]\++text++`) becomes one
/// [`Unescape`](MacroMatchKind::Unescape) that drops a backslash and leaves
/// the rest — the attrlist brackets included — literal, exactly like an
/// unattrlisted delimiter escape. A *bracket* escape (`\[attrs]++text++`)
/// becomes a **pair**: an [`Unescape`](MacroMatchKind::Unescape) over the
/// bracket, then a [`Node`](MacroMatchKind::Node) over the delimited
/// remainder, which the string replacer stores without its attribute list.
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
        // match crossing an already-recognized construct cannot occur here —
        // this is the very first step — but the check is kept for the same
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
                // backslash and the whole match — attrlist brackets included
                // — stays literal here, mirroring `handle_quoted_text`'s
                // `escape_count > 0` branch, which never builds a passthrough
                // either. The bare-form second pass
                // (`apply_bare_attrlisted_pass_level`) then legitimately
                // re-scans this now-literal, unopaqued text and may recognize
                // its own (different) match in it — exactly what the string
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
                // `[attrs]` prefix, and the delimited text after it is still
                // recognized — as an *ordinary* (non-attrlisted)
                // passthrough, since `handle_quoted_text` drops the
                // attribute list on this branch rather than storing it. That
                // is one match's worth of source doing two things, which is
                // exactly what a **pair** of matches expresses: an
                // `Unescape` over the bracket alone, then a `Node` over the
                // delimited remainder. `rebuild_macro_level` composes them
                // as it composes any two adjacent matches, so neither
                // `MacroMatchKind` variant needs to grow a new shape.
                //
                // Group 1 is the whole match's first byte whenever the
                // attrlist alternative participates (nothing in the pattern
                // precedes it), so the backslash offset is the match start;
                // `unwrap` is safe for the same reason `caps[1]` above is.
                #[allow(clippy::unwrap_used)]
                let escape = caps.get(1).unwrap();

                // The opening delimiter begins the recognized remainder.
                // Exactly one of groups 4/7/10 participates whenever a
                // delimited alternative matches at all.
                #[allow(clippy::unwrap_used)]
                let boundary = caps
                    .get(4)
                    .or_else(|| caps.get(7))
                    .or_else(|| caps.get(10))
                    .unwrap();

                let delimited = boundary.start()..full.end;

                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: escape.start(),
                    },
                    full: full.start..delimited.start,
                });

                let node = build_passthrough_node(&caps, &delimited, pieces, root, parser);

                matches.push(MacroMatch {
                    kind: MacroMatchKind::Node {
                        consumed: delimited.clone(),
                        node: Box::new(node),
                    },
                    full: delimited,
                });

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

/// Finds every bare passthrough at this level — the two attribute-list-
/// prefixed [`INLINE_PASS`] options (`` [x-]`text` `` and `[attrs]+text+`)
/// and the bare unconstrained form with no attribute list (`+text+`, built by
/// [`build_bare_unconstrained_match`]).
///
/// The scan is [`collect_bare_pass_matches`], which is recursive so that the
/// two attribute-list-prefixed options can reproduce the string replacer's
/// own **prohibited-prefix retry**; see its doc comment.
fn find_bare_attrlisted_matches<'src>(
    s: &str,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    collect_bare_pass_matches(s, 0..s.len(), nodes, pieces, root, parser, &mut matches);

    matches
}

/// Scans `s[region]` for [`INLINE_PASS`] matches, appending each to `matches`
/// in increasing order of `full.start` (what [`rebuild_macro_level`] wants).
/// Every offset a capture reports is relative to the scanned slice, so
/// `region.start` is added back to reach the level-wide offsets a
/// [`MacroMatch`] carries.
///
/// The two attribute-list-prefixed options need a lookbehind the string
/// replacer works around with a **retry** (`InlinePassReplacer`'s "prohibited
/// prefix" check, whose comment names the missing lookaround as its reason): a
/// match immediately preceded by `\`, `:`, or `;` is not really a passthrough,
/// so the replacer writes the match's *first character* back verbatim and runs
/// `INLINE_PASS.replace_all` again over the rest of that same match. That
/// second scan is not a no-op — it routinely recognizes a *different*,
/// shorter construct inside the rejected match, most often the bare
/// unconstrained form the leading `[` was hiding (`index:[attrs]+text+` →
/// a literal `[attrs]` and a bare passthrough over `text`) — which is why the
/// retry is reproduced here rather than approximated by leaving the whole
/// match literal.
///
/// This function *is* that retry: the prohibited case recurses over the same
/// match minus its first character, and the `[` it leaves behind is emitted
/// by [`rebuild_macro_level`] as an ordinary gap, since no match covers it.
/// Both options begin with `\[` (nothing in either alternative precedes the
/// bracket), so the character split off is always that one ASCII byte.
/// Recursion terminates because each retry region is strictly shorter than
/// the match that produced it.
///
/// The prefix test reads the level string's own preceding byte where the
/// replacer tests its *output* so far — the same one-byte approximation this
/// scan has always made, and exact wherever the two agree. At the start of a
/// retry region it is exact by construction: the byte before is the `[` the
/// retry just split off, which is not a prohibited character (the replacer's
/// freshly-empty `dest` likewise rejects nothing there).
///
/// The bare unconstrained form needs no retry at all — see
/// [`build_bare_unconstrained_match`].
#[allow(clippy::too_many_arguments)]
fn collect_bare_pass_matches<'src>(
    s: &str,
    region: std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    matches: &mut Vec<MacroMatch<'src>>,
) {
    // `region` is always a byte range this function itself derived from a
    // match on `s`, so it is in bounds and on character boundaries and `get`
    // always succeeds. The fallback is not a case to handle: an empty
    // haystack yields no captures, which is exactly what a region naming no
    // text should contribute.
    let haystack = s.get(region.clone()).unwrap_or_default();
    let offset = region.start;

    for caps in INLINE_PASS.captures_iter(haystack) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = (offset + whole.start())..(offset + whole.end());

        let is_backtick = caps.get(1).is_some();
        let is_plus_attrlisted = caps.get(3).is_some();

        if !is_backtick && !is_plus_attrlisted {
            // Option 3: the bare unconstrained form (no attribute list).
            if let Some(m) =
                build_bare_unconstrained_match(&caps, &full, offset, nodes, pieces, root, parser)
            {
                matches.push(m);
            }

            continue;
        }

        if full
            .start
            .checked_sub(1)
            .and_then(|i| s.as_bytes().get(i))
            .is_some_and(|b| matches!(b, b'\\' | b':' | b';'))
        {
            // Honor the prohibited prefix by retrying over this match minus
            // its leading `[`, exactly as `InlinePassReplacer` does.
            //
            // This sits *ahead* of the `range_is_verbatim` gate below, which
            // it used to follow: what the retry recognizes is a sub-range of
            // this match, and that sub-range can be verbatim where the whole
            // match is not (an opaque piece inside the attribute list the
            // retry is about to leave literal, say). Nothing is admitted
            // unchecked by the move — every match the retry does produce
            // passes this same gate, or `build_bare_unconstrained_match`'s
            // own copy of it, on its own range.
            collect_bare_pass_matches(
                s,
                (full.start + 1)..full.end,
                nodes,
                pieces,
                root,
                parser,
                matches,
            );

            continue;
        }

        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        if is_plus_attrlisted {
            let escape_count = caps.get(4).map_or(0, |m| m.len());

            if escape_count > 0 {
                // `[attrs]\+text+`: honor the escape of the formatting mark —
                // one backslash drops, the rest (attrlist brackets included)
                // stays literal, mirroring `InlinePassReplacer`'s own
                // `escape_count > 0` branch, which never builds a
                // passthrough here.
                #[allow(clippy::unwrap_used)]
                let group4 = caps.get(4).unwrap();

                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: offset + group4.start(),
                    },
                    full,
                });

                continue;
            }
        }

        let node = build_bare_attrlisted_passthrough_node(
            &caps,
            &full,
            offset,
            is_backtick,
            pieces,
            root,
            parser,
        );

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }
}

/// Builds one [`MacroMatch`] for a bare unconstrained [`INLINE_PASS`] match
/// (`+text+`, option 3 — no attribute list): a plain [`Raw`](InlineNode::Raw)
/// leaf, computed through the real substitution pipeline under
/// [`SubstitutionGroup::Verbatim`] exactly like the double-plus/double-dollar
/// forms (an absent attrlist means no stored `type_`, so
/// `PassthroughRestoreReplacer` never wraps the restored text in a rendered
/// span — unlike [`build_bare_attrlisted_passthrough_node`]'s `Styled` result).
///
/// Group 8 (the body) always participates whenever this alternative matches at
/// all, and the pattern's own leading/trailing `+` delimiters sit exactly one
/// byte to either side of it, so their offsets are derived rather than
/// captured separately. The optional boundary character the pattern consumes
/// ahead of the leading `+` (Group 6 — absent only when the match sits at the
/// very start of the level, via the pattern's `^` alternative) is not part of
/// the construct itself; it is kept as literal text before the node via the
/// same kept-prefix [`MacroMatchKind::Node`] sub-range the auto-link increment
/// introduced for a bare URL's own boundary prefix.
///
/// An escaped mark (`\+text+`, Group 7) drops the single backslash and keeps
/// the rest of the match — the boundary character included — as literal text,
/// with nothing left to re-scan it afterward (this is already the last pass),
/// so it is plain parity rather than a divergence.
///
/// # A body enclosing an **already-extracted** passthrough
///
/// This pass runs second, over what the [`INLINE_PASS_MACRO`] pass left
/// behind, so a `+…+` body can enclose a construct that pass already replaced
/// — `+a $$b$$ c+`, `+you feel pass:q[`mono`].+`, both documented AsciiDoc
/// idioms. The string pipeline sees its own **sentinel** there and treats it
/// as ordinary body text: it applies the verbatim substitution to the body
/// *with the sentinel still in it*, stores the result as this passthrough's
/// own entry, and lets the final restore splice the inner body in afterwards.
///
/// The tree reproduces that order exactly. The body is read from the level's
/// **match string** — where an already-built [`Raw`](InlineNode::Raw) or
/// [`Stem`](crate::inlines::Stem) leaf stands as one
/// [`SPAN_PLACEHOLDER`](super::quotes::SPAN_PLACEHOLDER), the same shape the
/// sentinel has —
/// [`passthrough_text`] runs over those bytes as written, and each placeholder
/// in the *result* is then replaced by what the fold of that node emits
/// ([`restorable_body`]). Substituting first and splicing after is what keeps
/// an inner `<b>` from being escaped a second time; a placeholder passes
/// through the verbatim substitution unchanged, so the two agree position for
/// position.
///
/// The gate is correspondingly [`range_is_restorable`]: a masked construct is
/// admitted, and so is a [`synthesized`](Piece::synthesized) run (the match
/// string carries its bytes exactly, and this no longer slices `'src` for the
/// body). Only the node's `location` keeps design §4.4's coarse span. Nothing
/// else can reach this pass — it runs before the escaping, quotes, and macros
/// steps, so a [`CharRef`](InlineNode::CharRef) leaf or a rendered span does
/// not exist yet.
///
/// `offset` is where the scanned slice starts in the level's match string —
/// non-zero inside a [`collect_bare_pass_matches`] retry — and is added back
/// to every capture offset this reads.
fn build_bare_unconstrained_match<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    offset: usize,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    if !range_is_restorable(nodes, pieces, full) {
        return None;
    }

    // The body (option 3's mandatory capture whenever this alternative
    // matches) is always preceded immediately by the construct's opening `+`.
    #[allow(clippy::unwrap_used)]
    let body_m = caps.get(8).unwrap();
    let body = (offset + body_m.start())..(offset + body_m.end());
    let delim_start = body.start - 1;

    if let Some(escape) = caps.get(7) {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: offset + escape.start(),
            },
            full: full.clone(),
        });
    }

    let consumed = delim_start..full.end;
    let location = source_slice(pieces, consumed.clone(), root);

    let (value, form) = substitute_and_restore(body_m.as_str(), &body, nodes, pieces, parser);

    Some(MacroMatch {
        kind: MacroMatchKind::Node {
            consumed,
            node: Box::new(InlineNode::Raw {
                value: CowStr::from(value),
                form,
                // The bare `+…+` form is `Verbatim` like its delimited
                // siblings; `substitute_and_restore` has already applied it,
                // so `value` is the author's body (or, for a body enclosing an
                // already-extracted construct, that body with each inner
                // node's own fold bytes spliced in — which is what the string
                // pipeline's own restore produces there too).
                origin: RawOrigin::Passthrough {
                    subs: SubstitutionGroup::Verbatim,
                    source_text: None,
                },
                location,
            }),
        },
        full: full.clone(),
    })
}

/// Applies the verbatim substitution to a bare `+…+` body and splices each
/// already-extracted node's own fold bytes back in — the restore-last order
/// the string pipeline itself performs, where the sentinel it holds for such a
/// node is ordinary body text until the final restore.
///
/// `body_text` is the body as it stands in the level's **match string** and
/// `range` is where that body sits in it, so the overlapping [`Piece`]s say
/// exactly which of its bytes are an extracted node's
/// [`SPAN_PLACEHOLDER`](super::quotes::SPAN_PLACEHOLDER) and which are ordinary
/// text. That distinction cannot
/// be recovered from the substituted string: the placeholder is an ordinary
/// (control) character a source can spell **literally**
/// (`+b\u{10}c+`), and a scan of the substituted bytes would read the two
/// alike — splicing a body at the literal one, and dropping the real node's.
///
/// So the walk is by piece rather than by character. Each run of ordinary
/// text between two restorable pieces is substituted on its own and appended,
/// then the piece's restored body is appended verbatim. Substituting run by
/// run gives the same bytes as substituting the whole body at once, because
/// the verbatim group is `specialcharacters` alone — a per-character map, so
/// it distributes over concatenation and no match can span a run boundary.
///
/// A body enclosing no restorable piece is simply the substituted body, which
/// is every `+…+` that was already at parity.
fn substitute_and_restore(
    body_text: &str,
    range: &std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    parser: &Parser,
) -> (String, RawForm) {
    let renderer = parser.renderer.as_ref();

    // The overwhelmingly common case: a `+…+` body with nothing extracted
    // inside it. Then there is no interleaving to do — the body is one
    // `Verbatim` run, which is to say the author's literal text — so it takes
    // the same [`Escaped`](RawForm::Escaped) form every other
    // specialcharacters-only body does, and the fold escapes it with the
    // renderer it is given rather than the one this parse happens to carry.
    //
    // The test is `node_is_restorable` rather than `restorable_body`, though
    // the two answer for the same set: this is a *discriminant*, and
    // `restorable_body` produces bytes — rendering a `Stem` body, escaping an
    // [`Escaped`](RawForm::Escaped) one — through the parser's renderer. Asking
    // it here would run those calls a second time for every node the
    // restoration loop below then renders for real, which a stateful renderer
    // reports (its second answer is what lands in the value) and a renderer
    // with side effects performs twice.
    if !pieces.iter().any(|piece| {
        piece.s_start + piece.s_len > range.start
            && piece.s_start < range.end
            && nodes.get(piece.node_index).is_some_and(node_is_restorable)
    }) {
        return (body_text.to_string(), RawForm::Escaped);
    }

    // Otherwise the value genuinely interleaves escaped text with another
    // node's *fold* bytes, and no single form describes it: it is built here
    // and emitted as-is. That keeps this one frozen against the parse's
    // renderer, which is the residue this shape owes to `render_with` — the
    // same debt a `pass:c,q[…]` body and a `Stem` body carry.
    let mut out = String::new();
    let mut cursor = range.start;

    for piece in pieces {
        let p_end = piece.s_start + piece.s_len;

        if p_end <= range.start || piece.s_start >= range.end {
            continue;
        }

        let Some(body) = nodes
            .get(piece.node_index)
            .and_then(|node| restorable_body(node, renderer))
        else {
            continue;
        };

        // The gate (`range_is_restorable`) admits only a body whose every
        // restorable piece lies wholly inside it, so a piece reaching here is
        // within `range` and at or after the cursor: the run is always a real
        // slice, and `unwrap_or_default` states that without adding a branch
        // no test could reach.
        let run = body_text
            .get(cursor.saturating_sub(range.start)..piece.s_start.saturating_sub(range.start))
            .unwrap_or_default();

        // `Verbatim` is `SpecialCharacters` alone, which records no warning,
        // so this run needs no anchoring — and has none available: it is a
        // slice of an interleaved body that may already be owned.
        out.push_str(&passthrough_text(
            Span::new(run),
            &SubstitutionGroup::Verbatim,
            parser,
        ));

        out.push_str(body.as_ref());
        cursor = p_end;
    }

    let tail = body_text
        .get(cursor.saturating_sub(range.start)..)
        .unwrap_or_default();

    out.push_str(&passthrough_text(
        Span::new(tail),
        &SubstitutionGroup::Verbatim,
        parser,
    ));

    (out, RawForm::AsIs)
}

/// Builds one [`Raw`](InlineNode::Raw) node from a verbatim, unescaped
/// passthrough match — see [`apply_passthroughs`] for how each delimiter form
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
            form: RawForm::AsIs,
            origin: RawOrigin::Passthrough {
                subs: SubstitutionGroup::None,
                source_text: None,
            },
            location,
        };
    }

    if let Some(m) = caps.get(8).or_else(|| caps.get(11)) {
        // `++text++` / `$$text$$`: `SubstitutionGroup::Verbatim` applies only
        // special characters — so the body is the author's *literal text*,
        // not raw output, and the fold is what escapes it
        // ([`RawForm::Escaped`]). Rendering it here instead would freeze it
        // against whichever renderer the parse carried, which is a different
        // renderer from the one a later `render_with` fold is handed, and
        // (while the string pipeline still runs) invokes that renderer a
        // second time for a value nothing reads.
        let content = source_slice(pieces, m.start()..m.end(), root);

        return InlineNode::Raw {
            value: CowStr::from(content.data()),
            form: RawForm::Escaped,
            origin: RawOrigin::Passthrough {
                subs: SubstitutionGroup::Verbatim,
                source_text: None,
            },
            location,
        };
    }

    // The `pass:` macro (no delimiters). With an **explicit substitution
    // list** (`pass:c,q[…]`, group 14), the body is rendered through the
    // real substitution pipeline under the resolved `SubstitutionGroup::
    // Custom` list — see the explicit-list arm below for why this (and
    // not a richer node subtree) is the safe shape for this increment.
    // Without one (the bare `pass:[…]` form), `SubstitutionGroup::None`
    // applies nothing. Either way, an escaped closing bracket (`\]`)
    // unescapes first, mirroring the string replacer's
    // `text.replace("\\]", "]")` — the same treatment every other macro
    // family's bracket content gets.
    #[allow(clippy::unwrap_used)]
    let m = caps.get(15).unwrap();
    let content = source_slice(pieces, m.start()..m.end(), root);
    let raw = content.data();
    let unescaped = raw.contains("\\]").then(|| raw.replace("\\]", "]"));

    // A `pass:` macro carrying an **explicit substitution list** is the one
    // deferred form 5a documented that a bare `SubstitutionGroup::None`/
    // `::Verbatim` treatment cannot cover, since the list can name *any* of the
    // six named steps, in any order and combination the author writes.
    //
    // A naive extension would thread `text` through this module's own node
    // transducers under the resolved step list, the way the legacy `x-`
    // compatibility marker's body does (`apply_normal_subs`). That shape does
    // not work here: `build` always runs its own fixed *normal* order over the
    // level this passthrough is embedded in, so any structural node (`Styled`,
    // `Ref`, …) this construct's own resolved subset produced would be visited
    // *again* by whichever of `build`'s six steps come after this one — and
    // unlike `Quotes` (whose delimiters are consumed, so a second pass finds
    // nothing left to match) or `SpecialCharacters` (whose `CharRef` leaves are
    // atomic), a macro's own display text is not idempotent under a second
    // pass: a `Ref{Link}`'s display children are literal text that
    // *looks* exactly like the source URL, so a second `Macros` pass would
    // recognize it all over again and nest a nested link inside it. A resolved
    // list omitting a step `build`'s own fixed order still runs (e.g.
    // `pass:q[<b>]`, which never asks for `SpecialCharacters`) has the same
    // problem in reverse: `build`'s own unconditional `SpecialCharacters` step
    // would escape content the author's list deliberately left raw.
    //
    // `passthrough_text` — already used for `++…++`/`$$…$$`/the bare
    // unconstrained form — sidesteps both failure modes: it renders `text`
    // through the **real, string-based** substitution pipeline
    // (`SubstitutionGroup::apply`, the same call `PassthroughRestoreReplacer`
    // makes for a stored `Passthrough`), producing an already-final HTML string
    // that this arm wraps in a single `Raw` leaf. A `Raw` leaf is *opaque* to
    // every later step in this module (never descended into, never re-matched —
    // design §4.2's passthrough-as-leaf convention), so it is immune to both
    // failure modes above: nothing in `build`'s own remaining steps can touch
    // it, whether or not the author's list included that step.
    //
    // That opacity is also why the explicit-list form is the one place `value`
    // is not the author's own bytes, and so the one place the node records a
    // `source_text`. Rendering an arbitrary group needs a `Parser` — to build
    // the body's own tree under that group (`passthrough_text`) — and a fold
    // takes a renderer and a `RenderContext` rather than a `Parser`, so the
    // body is rendered here and the input kept beside the result.
    let (value, subs, source_text) = if let Some(subs_list) = caps.get(14) {
        let text = unescaped.as_deref().unwrap_or(raw);
        let (subs, invalid) = SubstitutionGroup::from_custom_string(None, subs_list.as_str());

        // An unrecognized name in the list (`pass:bogus[…]`) is skipped while
        // the recognized ones are still honored — and reported, exactly as
        // `InlinePassMacroReplacer` reports it, against the content's own span.
        // Recorded here rather than replayed from the tree because the node
        // carries no trace of it: an invalid name leaves the value it produces
        // indistinguishable from a valid list's.
        if !invalid.is_empty() {
            parser.record_builder_diagnostic(
                root,
                crate::warnings::WarningType::InvalidSubstitutionTypeForPassthroughMacro(
                    invalid.join(", "),
                ),
            );
        }

        // Anchored on the body's own span so a warning the substitution
        // records points at the reference in the document (see
        // `passthrough_text`). An escaped `\]` shifts every byte after it, so
        // an unescaped copy has no anchor to offer and keeps the unlocated
        // form this whole shape used to have.
        let located = match &unescaped {
            Some(_) => Span::new(text),
            None => content,
        };

        (
            CowStr::from(passthrough_text(located, &subs, parser)),
            subs,
            Some(text.to_string()),
        )
    } else if let Some(unescaped) = unescaped {
        (CowStr::from(unescaped), SubstitutionGroup::None, None)
    } else {
        (CowStr::from(raw), SubstitutionGroup::None, None)
    };

    // Either the body under `SubstitutionGroup::None` (nothing applied, so the
    // author's bytes are the output bytes) or a value already rendered through
    // an explicit substitution list — which stays a frozen value, the
    // deferral the explicit-list arm above documents. Both are `AsIs`.
    InlineNode::Raw {
        value,
        form: RawForm::AsIs,
        origin: RawOrigin::Passthrough { subs, source_text },
        location,
    }
}

/// Builds one [`Styled`] node from a verbatim, unescaped, attribute-listed
/// `INLINE_PASS_MACRO` match (`[attrs]+++text+++`, `[attrs]++text++`,
/// `[attrs]$$text$$`) — the delimited half of the attribute-list-prefixed
/// forms this increment recognizes (see
/// [`build_bare_attrlisted_passthrough_node`] for the bare half). Folds through
/// the same `render_styled` `PassthroughRestoreReplacer` calls
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
    // all) — exactly one of groups 5/8/11 is therefore always `Some` here,
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

    // The wrapper is what the extraction pass records as one entry, so the
    // record rides here. Under the `x-` compatibility marker the body goes
    // through the **normal** substitutions as a subtree, which is both why the
    // group is `Normal` there and why no child could carry it.
    let subs = if old_behavior {
        SubstitutionGroup::Normal
    } else if boundary == "+++" {
        SubstitutionGroup::None
    } else {
        SubstitutionGroup::Verbatim
    };

    let children = if old_behavior {
        apply_normal_subs(body_span, parser)
    } else {
        // `+++` applies nothing, so its body is raw output; `++`/`$$` apply
        // special characters, so theirs is literal text the fold escapes. Both
        // carry the author's bytes unchanged — only the form differs.
        let form = if boundary == "+++" {
            RawForm::AsIs
        } else {
            RawForm::Escaped
        };

        vec![InlineNode::Raw {
            value: CowStr::from(body_span.data()),
            form,
            // This `Raw` is the *body* of an attribute-list-prefixed
            // passthrough, whose `Styled` wrapper is what the extraction pass
            // records as one entry; the group here is the body's own, read off
            // the delimiters exactly as the unprefixed forms above read theirs.
            origin: RawOrigin::Passthrough {
                subs: if form == RawForm::AsIs {
                    SubstitutionGroup::None
                } else {
                    SubstitutionGroup::Verbatim
                },
                source_text: None,
            },
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
        passthrough: Some(PassthroughWrapper {
            subs,
            text: body_span.data().to_string(),
        }),
        location,
    })
}

/// Builds one [`Styled`] node from a verbatim, unescaped, attribute-listed
/// bare [`INLINE_PASS`] match — either the backtick form (`` [x-]`text` ``,
/// `is_backtick`) or the plus form (`[attrs]+text+`).
///
/// The backtick form's attrlist is *always* `x-`-eligible — the regex itself
/// requires it (`` INLINE_PASS ``'s option 1 only matches `[x-]` or
/// `[… x-]`) — but its format mark (`` ` ``) keeps `subs` at `Verbatim`
/// regardless, mirroring `InlinePassReplacer`'s `format_mark != '`'` guard:
/// only the plus form's `old_behavior` switches to the `Normal` group.
///
/// `offset` is where the scanned slice starts in the level's match string —
/// non-zero inside a [`collect_bare_pass_matches`] retry — and is added back
/// to every capture offset this reads.
fn build_bare_attrlisted_passthrough_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    offset: usize,
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
    let attrlist_span = source_slice(
        pieces,
        (offset + attrlist_m.start())..(offset + attrlist_m.end()),
        root,
    );

    // Both the backtick body (group 2, requiring at least one non-space
    // character) and the plus body (group 5) are mandatory captures of
    // whichever alternative matched — never a genuinely absent one.
    #[allow(clippy::unwrap_used)]
    let body_m = body.unwrap();
    let body_span = source_slice(
        pieces,
        (offset + body_m.start())..(offset + body_m.end()),
        root,
    );

    let (attrlist_span, old_behavior) = split_old_behavior_attrlist(attrlist_span);

    let variant = if old_behavior {
        StyleVariant::Code
    } else {
        StyleVariant::Unquoted
    };

    // As above: the wrapper carries the record, and the compat marker's
    // subtree body is why it cannot live on a child.
    let subs = if old_behavior && !is_backtick {
        SubstitutionGroup::Normal
    } else {
        SubstitutionGroup::Verbatim
    };

    let children = if old_behavior && !is_backtick {
        apply_normal_subs(body_span, parser)
    } else {
        // Always `SubstitutionGroup::Verbatim` here (see this function's own
        // doc comment), so the body is literal text the fold escapes.
        vec![InlineNode::Raw {
            value: CowStr::from(body_span.data()),
            form: RawForm::Escaped,
            origin: RawOrigin::Passthrough {
                subs: SubstitutionGroup::Verbatim,
                source_text: None,
            },
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
        passthrough: Some(PassthroughWrapper {
            subs,
            text: body_span.data().to_string(),
        }),
        location,
    })
}

/// Splits an old-behavior-eligible attrlist span into its final attrlist body
/// and whether the legacy `x-` compatibility marker was present — mirroring
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
/// list includes `Macros` — which `Normal`'s does — so a passthrough or STEM
/// macro nested in an `x-` body (`[x-]++pass:[<b>]++`) is itself extracted
/// and restored, not left for `Macros` to walk over as plain text.
/// [`build`](super::build)
/// already threads a span through exactly that full sequence — passthroughs,
/// STEM, then the six steps, footnotes included (the string pipeline's
/// `Macros` step recognizes footnote macros too, so `Normal`'s semantics
/// cover them; `build` only splits that recognition into its own step for
/// numbering-order reasons, design §5.2 step 4b(ii) part 4c) — so this
/// delegates to it directly rather than re-deriving a subset.
fn apply_normal_subs<'src>(text: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
    super::build(text, parser, None)
}

/// Renders `text` under `subs` **through the tree**, returning the resulting
/// owned string. Used to compute a [`Raw`](InlineNode::Raw) passthrough's
/// `value` — for [`SubstitutionGroup::Verbatim`], for a `stem:` body, and for
/// the explicit substitution list a `pass:c,q[…]` carries — so the result
/// honors whatever
/// [`InlineRenderer`](crate::parser::InlineRenderer)
/// `parser` carries rather than a hand-rolled, always-default escaping.
///
/// **This is the authoritative-pass closure** (design §5.2's step 6). Until
/// now this ran `subs.apply`, which re-entered
/// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup) for the
/// body; that re-entry took no tree seed (a reentrancy guard on the `Parser`,
/// since retired along with the re-entry), so its *string* pipeline was the
/// body's authoritative pass. It was the last thing
/// in production keeping `run_pipeline` alive — the survey named it the
/// only non-test-side blocker to the deletion, which has since landed.
///
/// It closes because [`build_for_group`](super::build_for_group) already runs
/// **an arbitrary group's steps in that group's own order** — including a
/// `Custom` order that puts the escaping step after a step that produced
/// markup, which is what `flatten_prior_markup` and `SplicedSpecials` are for.
/// So the body needs no string pipeline of its own: it is built as a tree and
/// folded, exactly as every other content is, and the enclosing level goes on
/// wrapping the result in one opaque `Raw` leaf.
///
/// *The obvious objection does not apply here, which is why this works.* A
/// passthrough's body cannot be built as *nodes spliced into the enclosing
/// level* — the enclosing [`build`](super::build) runs its own fixed normal
/// order over that level, so any structural node the body's own resolved
/// subset produced would be visited again by whichever steps come after, and
/// a macro's display text is not idempotent under a second pass. That
/// objection is about **splicing**, not about computing: folding the body's
/// own tree to a string and wrapping it in a `Raw` leaf keeps it opaque to
/// every later step, which is the same containment `subs.apply` bought.
///
/// `text` is a [`Span`] rather than a `&str` so that a warning this body's
/// substitution records lands on the reference's **own** position in the
/// document — the answer #1301 settled. It stays true through the tree: the
/// span is the build's `location`, so the reference's node carries it and the
/// diagnostic is raised against it. The per-line spans that call needed are
/// gone with it; they existed because `apply_attributes` scanned a *string*
/// and had only the whole-body span to fall back on, and a tree has each
/// reference's own node instead.
pub(super) fn passthrough_text(
    text: Span<'_>,
    subs: &SubstitutionGroup,
    parser: &Parser,
) -> String {
    let nodes = super::build_for_group(subs, CowStr::from(text.data()), text, parser, None);

    super::fold::fold_html(&nodes, &*parser.renderer, &parser.render_context())
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
            assert_raw, assert_styled, build_src, fold_html, golden_passthroughs, passthrough, seed,
        },
        apply_pass_macro_level, apply_passthroughs,
    };
    use crate::{
        HasSpan, Parser, Span,
        inlines::{InlineNode, SpanForm, StyleVariant, Styled},
        parser::HtmlInlineRenderer,
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
        // false branch is defensive — kept for the same reason every other
        // macro family keeps the check. Exercise it directly, feeding a
        // hand-built level whose triple-plus content spans an already-built
        // `Styled` node, to document the intended fallback: the whole match
        // is left unrecognized rather than mis-sliced.
        //
        // This calls `apply_pass_macro_level` (not the top-level
        // `apply_passthroughs`) to isolate this guard from the bare
        // unconstrained pass that runs after it: each leftover `+++` run is,
        // on its own, itself a valid (non-crossing) bare passthrough — see
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
                attrs: crate::attributes::Attrlist::empty(location.slice(0..0)),
                children: vec![],
                passthrough: None,
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
        // raw — not even special characters are escaped — and borrows `'src`.
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
        // in front of `text+++` — exactly what the string pipeline's own
        // second regex pass does over its own once-substituted text, so this
        // is parity, not a divergence.
        let source = r"\+++text+++";
        let nodes = build_src(Span::new(source));

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
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

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
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

        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), "pass:[x]");
    }

    #[test]
    fn a_bare_plus_body_is_literal_text_unless_it_restores_something() {
        use super::super::test_support::assert_raw_form;
        use crate::inlines::RawForm;

        // A bare `+…+` body is `SubstitutionGroup::Verbatim` like `++…++`, so
        // by rights it is the author's literal text too. It could not say so
        // while one shape stood in the way: a body *enclosing a construct the
        // first extraction pass already replaced* interleaves escaped text with
        // that node's own **fold** bytes, and no single form describes the
        // mixture.
        //
        // That shape is rare and the plain one is not, so the mixture is
        // detected rather than assumed: with nothing restorable inside the
        // body, the value is the author's bytes and the fold escapes them.
        let nodes = build_src(Span::new("+a < b+"));
        assert_raw_form(
            &nodes[0],
            RawForm::Escaped,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "a < b",
        );

        // The mixture keeps `AsIs`, since part of its value is another node's
        // rendering rather than anything an escape could reproduce.
        let nodes = build_src(Span::new("+a $$<b>$$ c+"));
        assert_raw_form(
            &nodes[0],
            RawForm::AsIs,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "a &lt;b&gt; c",
        );
    }

    #[test]
    fn a_bare_plus_body_honors_the_renderer_the_fold_is_given() {
        // The observable consequence, and the reason this is worth its own
        // increment rather than a documented deferral: `+…+` is a common
        // construct, and its body used to be escaped against whichever renderer
        // the *parse* carried.
        #[derive(Debug)]
        struct BracketRenderer;

        impl crate::parser::InlineRenderer for BracketRenderer {
            fn render_special_character(
                &self,
                type_: crate::parser::SpecialCharacter,
                dest: &mut String,
            ) {
                dest.push_str(match type_ {
                    crate::parser::SpecialCharacter::Lt => "[LT]",
                    crate::parser::SpecialCharacter::Gt => "[GT]",
                    crate::parser::SpecialCharacter::Ampersand => "[AMP]",
                });
            }
        }

        let parser = Parser::default();
        let nodes = build_src(Span::new("+a < b > c & d+"));

        assert_eq!(
            super::super::fold_html(&nodes, &HtmlInlineRenderer {}, &parser.render_context()),
            "a &lt; b &gt; c &amp; d"
        );

        assert_eq!(
            super::super::fold_html(&nodes, &BracketRenderer, &parser.render_context()),
            "a [LT] b [GT] c [AMP] d"
        );
    }

    #[test]
    fn a_specialcharacters_body_is_literal_text_the_fold_escapes() {
        use super::super::test_support::assert_raw_form;
        use crate::inlines::RawForm;

        // The shape this increment exists for. `SubstitutionGroup` decides
        // which form a passthrough body takes, and the two are not the same
        // thing wearing different labels:
        //
        //   - `+++…+++` and bare `pass:[…]` apply *nothing*, so the author's bytes are
        //     the output bytes — raw output by design.
        //   - `++…++` and `$$…$$` apply special characters and nothing else, so the
        //     body is the author's *literal text*. Escaping it is the fold's job, with
        //     whatever renderer the fold is given.
        //
        // Both stay opaque to every later step — that is what keeps them one
        // node kind — so this pins the distinction the kind alone cannot.
        let nodes = build_src(Span::new("+++<b>+++"));
        assert_raw_form(
            &nodes[0],
            RawForm::AsIs,
            passthrough(crate::content::SubstitutionGroup::None),
            "<b>",
        );

        let nodes = build_src(Span::new("pass:[<b>]"));
        assert_raw_form(
            &nodes[0],
            RawForm::AsIs,
            passthrough(crate::content::SubstitutionGroup::None),
            "<b>",
        );

        let nodes = build_src(Span::new("++<b>++"));
        assert_raw_form(
            &nodes[0],
            RawForm::Escaped,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "<b>",
        );

        let nodes = build_src(Span::new("$$<b>$$"));
        assert_raw_form(
            &nodes[0],
            RawForm::Escaped,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "<b>",
        );

        // The attribute-listed forms carry theirs as the `Styled` wrapper's
        // one child, and split the same way.
        let nodes = build_src(Span::new("[.role]+++<b>+++"));
        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw_form(
            &children[0],
            RawForm::AsIs,
            passthrough(crate::content::SubstitutionGroup::None),
            "<b>",
        );

        let nodes = build_src(Span::new("[.role]++<b>++"));
        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw_form(
            &children[0],
            RawForm::Escaped,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "<b>",
        );
    }

    #[test]
    fn a_passthrough_body_honors_the_renderer_the_fold_is_given() {
        // The defect this closes. A `++…++` body used to be escaped at *build*
        // time, through whichever renderer the `Parser` carried, and frozen
        // into the node. Two things followed, and both are wrong:
        //
        //   1. folding the tree with a *different* renderer — which is the whole point
        //      of `render_with` — silently emitted the parse-time renderer's escaping
        //      instead;
        //   2. building the tree *invoked* the document's renderer for a value nothing
        //      reads, so a renderer with state saw extra calls and a later block's
        //      authoritative output shifted under it.
        //
        // Folding the same tree through two different backends is the sharpest
        // statement of the fix: one tree, two renderings, neither frozen.
        #[derive(Debug)]
        struct BracketRenderer;

        impl crate::parser::InlineRenderer for BracketRenderer {
            fn render_special_character(
                &self,
                type_: crate::parser::SpecialCharacter,
                dest: &mut String,
            ) {
                dest.push_str(match type_ {
                    crate::parser::SpecialCharacter::Lt => "[LT]",
                    crate::parser::SpecialCharacter::Gt => "[GT]",
                    crate::parser::SpecialCharacter::Ampersand => "[AMP]",
                });
            }
        }

        let parser = Parser::default();
        let nodes = build_src(Span::new("++a < b > c & d++"));

        assert_eq!(
            super::super::fold_html(&nodes, &HtmlInlineRenderer {}, &parser.render_context()),
            "a &lt; b &gt; c &amp; d"
        );

        assert_eq!(
            super::super::fold_html(&nodes, &BracketRenderer, &parser.render_context()),
            "a [LT] b [GT] c [AMP] d"
        );

        // A genuinely raw body is not escaped by either backend — the other
        // half of the distinction, which a single-renderer test cannot show.
        let raw = build_src(Span::new("+++a < b+++"));

        assert_eq!(
            super::super::fold_html(&raw, &BracketRenderer, &parser.render_context()),
            "a < b"
        );
    }

    #[test]
    fn building_a_passthrough_does_not_invoke_the_documents_renderer() {
        // The second half of the defect above, pinned directly: a renderer with
        // state must not advance while the *tree* is built, because the tree is
        // derived and its build must not be observable. Before the fix,
        // `passthrough_text` ran the real substitution pipeline here and this
        // counter reached 1.
        // The counter has to be *observable in the output*, since a renderer
        // behind an `Rc<dyn …>` cannot be downcast to read a field: each call
        // emits its own ordinal, so probing afterwards reports how many calls
        // came before it.
        #[derive(Debug, Default)]
        struct OrdinalRenderer {
            calls: std::cell::Cell<usize>,
        }

        impl crate::parser::InlineRenderer for OrdinalRenderer {
            fn render_special_character(
                &self,
                _type_: crate::parser::SpecialCharacter,
                dest: &mut String,
            ) {
                self.calls.set(self.calls.get() + 1);
                dest.push_str(&format!("[{}]", self.calls.get()));
            }
        }

        let parser = Parser::default().with_inline_renderer(OrdinalRenderer::default());

        let _ = super::super::build(Span::new("++a < b++"), &parser, None);

        let mut probe = String::new();
        parser
            .renderer
            .render_special_character(crate::parser::SpecialCharacter::Lt, &mut probe);

        assert_eq!(
            probe, "[1]",
            "building the tree must not invoke the document's renderer; the probe is call one"
        );
    }

    #[test]
    fn a_mixed_bare_plus_body_renders_each_nested_node_once() {
        // A bare `+…+` enclosing an already-extracted node is the one shape
        // whose value is finished at build time, so it is also the one place
        // the builder legitimately calls the document's renderer. It must call
        // it *once* per nested node: the mixture test above it is a
        // discriminant (`node_is_restorable`), not a second rendering.
        //
        // The counter has to be observable in the output, since a renderer
        // behind an `Rc<dyn …>` cannot be downcast to read a field: each call
        // emits its own ordinal. Before the fix the detection pass rendered
        // the `$$…$$` node's `<` first, so the *second* answer (`[2]`) is what
        // landed in the value and the probe afterwards read `[3]`.
        #[derive(Debug, Default)]
        struct OrdinalRenderer {
            calls: std::cell::Cell<usize>,
        }

        impl crate::parser::InlineRenderer for OrdinalRenderer {
            fn render_special_character(
                &self,
                _type_: crate::parser::SpecialCharacter,
                dest: &mut String,
            ) {
                self.calls.set(self.calls.get() + 1);
                dest.push_str(&format!("[{}]", self.calls.get()));
            }
        }

        use super::super::test_support::assert_raw_form;
        use crate::inlines::RawForm;

        let parser = Parser::default().with_inline_renderer(OrdinalRenderer::default());

        let nodes = super::super::build(Span::new("+a $$b < c$$ d+"), &parser, None);

        assert_eq!(nodes.len(), 1);
        assert_raw_form(
            &nodes[0],
            RawForm::AsIs,
            passthrough(crate::content::SubstitutionGroup::Verbatim),
            "a b [1] c d",
        );
        assert_eq!(nodes[0].span().data(), "+a $$b < c$$ d+");

        let mut probe = String::new();
        parser
            .renderer
            .render_special_character(crate::parser::SpecialCharacter::Lt, &mut probe);

        assert_eq!(
            probe, "[2]",
            "the nested node must be rendered once; the probe is call two"
        );
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
            // An attribute list carrying a special character. Unlike an
            // attributed *quote's* — parsed from the escaped text, since the
            // escaping step runs before the quotes step (see
            // [`quote_attributes`](super::quotes)) — this extraction pass runs
            // *ahead* of every step, so the string pipeline parses the
            // author's raw bytes here and `attributes_of` matches it by
            // parsing the source slice.
            "[.a<b]++text++",
            "[#a&b]++text++",
            "['a<b&c']++text++",
            "[.a<b]+text+",
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
            // and leaves the rest literal — which the bare-form second pass
            // then legitimately re-recognizes as its own (different) match,
            // exactly as the string pipeline's own second regex pass does.
            r"[.role]\++text++",
            // The bare-plus form's own delimiter escape: dropped backslash,
            // literal remainder, no further pass to re-scan a residue.
            r"[attrs]\+text+",
            // A *bracket* escape is the other branch: the bracket unescapes
            // to a literal prefix and the delimited remainder is still a
            // passthrough — an ordinary one, so no `Styled` span and, for
            // `++`, no `x-` monospace either. Every boundary, in flow,
            // beside its unescaped twin, with a special character and with
            // quote syntax in the kept prefix (both substituted normally
            // there, exactly as the string pipeline substitutes its own
            // literal `[attrs]`), and spanning a newline.
            r"\[attrs]++text++",
            r"abc \[attrs]++text++",
            r"\[.role]+++*bold*+++",
            r"\[.role]$$<b>*x*</b>$$",
            r"\[x-]++*bold* and _em_++",
            r"\[attrs]++<b>*x*</b>++",
            r"a \[.role]++text++ b",
            r"\[attrs]++one++ and [attrs]++two++",
            r"\[a<b]++text++",
            r"\[.a*b*c]++text++",
            r"*bold* \[attrs]++text++ _em_",
            "multi\nline \\[x-]++text\nmore++ end",
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
            // The two *attribute-list-prefixed* options behind that same
            // prohibited prefix, which their own pattern does not exclude
            // and which the string replacer answers with a retry over the
            // match minus its leading `[`. For the plus option that retry
            // routinely finds the bare unconstrained form the bracket was
            // hiding — a literal `[attrs]` prefix and an *ordinary*
            // passthrough over the body, so no `Styled` span and no `x-`
            // monospace — while the backtick option finds nothing there and
            // leaves the whole construct to the later quotes step. All three
            // prefixes, the `x-` marker the retry's ordinary form drops, a
            // body carrying specials and quote syntax, an attribute list
            // carrying a special, in flow, spanning a newline, twice in one
            // flow, beside an unprefixed twin, and the delimiter escapes the
            // retry's own second scan honors.
            "index:[attrs]+text+",
            r"a\[attrs]+text+",
            "a;[attrs]+text+",
            "see index:[foo]+bar+ end",
            "index:[x-]+text+",
            "index:[method x-]+save()+",
            "index:[attrs]+<b>*x*</b>+",
            "index:[a<b]+text+",
            "index:[attrs]+multi\nline+",
            "index:[attrs]+text+ and [attrs]+text+",
            "a:[b]+x+ c:[d]+y+",
            r"index:[attrs]\+text+",
            r"index:[attrs]\\+text+",
            "x:[x-]`text`",
            r"a\[x-]`text`",
            "x;[method x-]`text`",
            // Writing *both* escapes at once: the delimiter escape wins the
            // first pass's branch, and the literal `++text++` it leaves
            // behind then sits behind exactly the `\` this second pass
            // declines — so it is the retry, not the ordinary scan, that
            // recognizes the shorter `+text+` inside it.
            r"\[attrs]\++text++",
        ];

        for source in fixtures {
            let nodes = build_src(Span::new(source));
            let folded = fold_html(&nodes, &HtmlInlineRenderer {});

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
        // body — not a plain `Raw` leaf.
        let nodes = build_src(Span::new("[.role]++text++"));

        assert_eq!(nodes.len(), 1);
        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);

        assert_eq!(children.len(), 1);
        assert_raw(&children[0], "text");

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.roles, vec![CowStr::from("role")]);
                assert_ne!(
                    styled.attrs.attributes().len(),
                    0,
                    "the attribute list is retained"
                );
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
        // characters — the same treatment the unattrlisted `++…++` form gets.
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
        // `Normal` substitution order — quotes included — unlike the
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
        // step list whenever `Macros` is in scope — which it is for `Normal` —
        // so a `pass:[…]` nested in the `x-` body is itself extracted and
        // restored by `apply_normal_subs`'s delegation to `build`, rather than
        // being left for `apply_macros` (which does not recognize `pass:` at
        // all) to walk over as plain text.
        let nodes = build_src(Span::new("[x-]++pass:[<b>]++"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_raw(&children[0], "<b>");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs("[x-]++footnote:[note text]++")
        );
    }

    #[test]
    fn an_x_dash_marker_with_a_leading_role_keeps_the_role() {
        // `[method x-]+save()+`: the trailing ` x-` is stripped, leaving
        // `method` as the surviving attrlist body. `styled.roles` (from
        // `Attrlist::roles`) does not itself capture a bare first positional
        // attribute like `method` — the renderer's own
        // `render_styled` treats it as a role via
        // `nth_attribute(1).block_style()`, using `styled.attrs` (kept in
        // full) rather than `styled.roles` — so this is asserted through the
        // fold, which is what the differential corpus also pins.
        let nodes = build_src(Span::new("[method x-]+save()+"));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.variant, StyleVariant::Code);
                assert_ne!(
                    styled.attrs.attributes().len(),
                    0,
                    "the attribute list is retained"
                );
            }

            other => panic!("expected Styled, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
                fold_html(&nodes, &HtmlInlineRenderer {}),
                r#"<span class="x-">text</span>"#,
                "for {source:?}"
            );
        }
    }

    #[test]
    fn the_backtick_bare_form_is_always_monospace_under_verbatim_subs() {
        // `` [x-]`just *mono*` ``: the backtick form's attrlist is always
        // `x-`-eligible (the regex itself requires it), but its format mark
        // keeps `subs` at `Verbatim` regardless — `*mono*` stays literal,
        // unlike the plus form's `Normal`-subs old-behavior case.
        let nodes = build_src(Span::new("[x-]`just *mono*`"));

        let children = assert_styled(&nodes[0], StyleVariant::Code, SpanForm::Unconstrained);
        assert_eq!(children.len(), 1);
        assert_raw(&children[0], "just *mono*");
    }

    #[test]
    fn the_plus_bare_form_without_x_dash_is_an_unquoted_span() {
        // `[.role]+text+`: an ordinary (non-`x-`) attrlist on the plus bare
        // form behaves like the delimited `++`/`$$` boundaries — `Unquoted`
        // under `Verbatim` subs.
        let nodes = build_src(Span::new("[.role]+text+"));

        let children = assert_styled(&nodes[0], StyleVariant::Unquoted, SpanForm::Unconstrained);
        assert_raw(&children[0], "text");
    }

    #[test]
    fn an_escaped_attrlist_bracket_keeps_its_prefix_and_builds_a_plain_raw_leaf() {
        // `\[attrs]++text++` is one match's source doing two things: the
        // bracket unescapes to a literal `[attrs]` prefix, and the delimited
        // text after it is still recognized — as an *ordinary*
        // (non-attrlisted) passthrough, since `handle_quoted_text` drops the
        // attribute list on this branch. The pair of matches that expresses
        // it yields a `Text` prefix and a plain `Raw` leaf, *not* the
        // `Styled` span an unescaped `[attrs]++text++` builds.
        let source = r"\[attrs]++text++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped attrlist bracket must not build a Styled span: {nodes:?}"
        );

        let raws: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Raw { .. }))
            .collect();

        assert_eq!(raws.len(), 1, "expected exactly one Raw leaf: {nodes:?}");
        assert_raw(raws[0], "text");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_real_documents_escaped_attrlist_brackets_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named
        // this increment: an escaped attribute-list bracket must keep its
        // literal prefix *and* build the delimited remainder, so a tree that
        // left the whole construct literal — or that wrapped the remainder in
        // the `Styled` span its unescaped twin gets — would regress the moment
        // `rendered_html()` becomes a fold of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            r"An escaped \[attrs]++<b>*x*</b>++ bracket.",
            "\n\n",
            r"The \[x-]++*bold*++ marker does not survive the escape, unlike [x-]++*bold*++.",
            "\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected every paragraph to carry a tree");
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_attrlisted_form_retries_over_the_rest() {
        // The string pipeline's own `InlinePassReplacer` retries around a
        // match immediately preceded by `\`, `:`, or `;` (no lookbehind in
        // Rust's regex engine): it writes the match's first character back
        // verbatim and re-scans the rest. That retry is not a no-op — here it
        // finds the bare unconstrained form the leading `[` was hiding — so
        // the attribute list stays a literal prefix and the body becomes an
        // *ordinary* passthrough: a plain `Raw` leaf, never the `Styled` span
        // the same source without the prefix builds.
        let source = "index:[attrs]+text+";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "a prohibited-prefix match must not build a Styled span: {nodes:?}"
        );

        let raws: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Raw { .. }))
            .collect();

        assert_eq!(raws.len(), 1, "expected exactly one Raw leaf: {nodes:?}");
        assert_raw(raws[0], "text");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_backtick_form_finds_nothing_to_retry() {
        // The retry's other half. Splitting `` [x-]`text` `` after its `[`
        // leaves `` x-]`text` ``, which neither `INLINE_PASS` option can
        // match (both attribute-list options need the bracket the retry just
        // consumed, and there is no `+` for the bare unconstrained one), so
        // the second scan contributes nothing and the whole construct is left
        // for the later quotes step — which is exactly what the string
        // replacer's own empty retry leaves behind, hence parity.
        for source in ["x:[x-]`text`", r"a\[x-]`text`", "x;[method x-]`text`"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
                "the retry must find no passthrough for {source:?}: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_passthroughs(source),
                "for {source:?}"
            );
        }
    }

    #[test]
    fn both_escapes_at_once_leave_the_retry_to_recognize_the_remainder() {
        // `\[attrs]\++text++` writes both of the attribute-list-prefixed
        // escapes at once. The *delimiter* escape wins the first pass's
        // branch (`handle_quoted_text`'s own `escape_count > 0` arm, which
        // never builds a passthrough), so what reaches the bare-form second
        // pass is a literal `\[attrs]++text++` — whose `[attrs]+…+` match
        // sits behind exactly the `\` that pass declines. Only the retry
        // reaches the `+text+` inside it, so this shape is the one that
        // fails outright without one.
        let source = r"\[attrs]\++text++";
        let nodes = build_src(Span::new(source));

        let raws: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Raw { .. }))
            .collect();

        assert_eq!(raws.len(), 1, "expected exactly one Raw leaf: {nodes:?}");
        assert_raw(raws[0], "+text");

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});

        assert_eq!(folded, r"\[attrs]+text+");
        assert_eq!(folded, golden_passthroughs(source));
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_attrlisted_form_keeps_its_source_offsets() {
        // The retry scans a *slice* of the level's match string, so every
        // offset its captures report is relative to that slice and has to be
        // rebased before it can name a source span. Pin that arithmetic where
        // it is observable: the `Raw` leaf's location must be the body's own
        // source bytes, not a range shifted left by the retry's start.
        let source = Span::new("see index:[foo]+bar+ end");
        let nodes = build_src(source);

        let raws: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Raw { .. }))
            .collect();

        assert_eq!(raws.len(), 1, "expected exactly one Raw leaf: {nodes:?}");
        assert_raw(raws[0], "bar");

        // `+bar+`, the construct the retry recognized — the boundary `]`
        // ahead of it stays literal, exactly as it does without a prefix.
        assert_eq!(raws[0].span(), source.slice(15..20));
    }

    #[test]
    fn a_real_documents_prohibited_prefix_passthroughs_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named
        // this increment: a tree that skipped the retry would leave
        // `index:[attrs]+text+` entirely literal and regress the moment
        // `rendered_html()` becomes a fold of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "The index:[attrs]+text+ form keeps its bracket, unlike [attrs]+text+.",
            "\n\n",
            r"Writing both escapes at once, \[attrs]\++text++, lands there too.",
            "\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected every paragraph to carry a tree");
    }

    #[test]
    fn a_bare_attrlisted_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // Exercises `find_bare_attrlisted_matches`'s own `range_is_verbatim`
        // guard directly — the second pass's counterpart to
        // `a_match_whose_content_crosses_an_already_built_node_is_deferred`.
        // Reconstructed as flat text this level would read `[attrs]+x+`, but
        // the single-character body sits on an already-built (opaque)
        // `Styled` node rather than verbatim text, so the candidate match —
        // whose `full` range still spans it — is left unrecognized.
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
                attrs: crate::attributes::Attrlist::empty(source.slice(8..9).slice(0..0)),
                children: vec![],
                passthrough: None,
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
        // which runs first) recognizes the embedded `pass:[<b>]` on its own —
        // `INLINE_PASS_MACRO`'s `pass:` alternative matches that substring
        // regardless of surrounding context — *before* the bare-form second
        // pass gets a chance to see `[method x-]+…+` as one attrlisted
        // construct. The candidate bare-form match's body then spans the
        // already-built (opaque) node the macro pass left behind, so
        // `range_is_verbatim` defers it — the real-world trigger for
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

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(golden, r#"<code class="method"><b></code>"#);
    }

    #[test]
    fn a_bare_plus_attrlisted_delimiter_escape_stays_literal() {
        // `[attrs]\+text+`: honors the escape of the formatting mark — one
        // backslash drops, the rest (attrlist brackets included) stays
        // literal, mirroring `InlinePassReplacer`'s own `escape_count > 0`
        // branch. Unlike the delimited form's own escape
        // (`[.role]\++text++`), there is no further pass to re-scan the
        // residue here — this second pass already is the last one — so the
        // result is simple parity with the golden string pipeline.
        let source = r"[attrs]\+text+";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped bare-attrlisted delimiter must not build a Styled node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert_eq!(folded, "[attrs]+text+");
        assert_eq!(folded, golden_passthroughs(source));
    }

    #[test]
    fn a_pass_macro_with_a_special_characters_subs_list_is_a_raw_node() {
        // `pass:c[…]` resolves to `Custom([SpecialCharacters])`: the body is
        // rendered through the real pipeline under just that one step (see
        // `build_passthrough_node`'s explicit-list arm), so `<`/`>` are
        // already escaped in the leaf's `value` — a single opaque `Raw` node,
        // not `CharRef` leaves this builder's own `SpecialCharacters`
        // transducer would produce.
        let source = "pass:c[<b>]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt;");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_with_a_quotes_subs_list_renders_the_markup() {
        // `pass:q[…]` resolves to `Custom([Quotes])`: rendered through the
        // real pipeline, so the leaf's `value` already carries the
        // `<strong>` markup `Quotes` produced — unescaped, since `Raw`'s
        // fold emits `value` verbatim.
        let source = "pass:q[*bold*]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "<strong>bold</strong>");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_pass_macro_with_multiple_subs_applies_them_in_the_order_given() {
        // `pass:q,c[…]` resolves to `Custom([Quotes, SpecialCharacters])` —
        // the order the author wrote, not the *normal* effective order
        // (which always runs `SpecialCharacters` first). `Quotes` runs
        // first here, wrapping `*bold*` in a literal `<strong>…</strong>`;
        // `SpecialCharacters` then runs *second* and escapes every `<`/`>`
        // it finds — tags included, since it has no way to tell them apart
        // from `<b>`'s own literal angle brackets. This is the documented
        // gotcha of naming `specialcharacters` after a step that emits
        // markup, reproduced byte-for-byte from the real pipeline.
        let nodes = build_src(Span::new("pass:q,c[<b> *bold*]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "&lt;b&gt; &lt;strong&gt;bold&lt;/strong&gt;");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs("pass:q,c[<b> *bold*]")
        );
    }

    #[test]
    fn a_pass_macro_with_an_unrecognized_subs_name_skips_it() {
        // An unrecognized name resolves to zero steps — rather than
        // invalidating the whole list — mirroring
        // `SubstitutionGroup::from_custom_string`/`InlinePassMacroReplacer`'s
        // own "skip and keep going" resolution. With no steps at all the
        // rendered `value` is the content completely untouched (not even
        // special characters are escaped).
        let source = "pass:bogus[<b>]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "<b>");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_recognized_name_beside_an_unrecognized_one_is_still_honored() {
        // `pass:c,bogus[…]` resolves to the same `Custom([SpecialCharacters])`
        // as `pass:c[…]` alone — the unrecognized name is skipped, not fatal
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
    fn fold_matches_the_string_pipeline_for_a_bare_form_over_an_extracted_passthrough() {
        // The bare `+…+` form runs in this step's *second* pass, so its body
        // can enclose a construct the first pass already replaced. The string
        // pipeline sees its own sentinel there and treats it as ordinary body
        // text — substituting over it and letting the final restore splice the
        // inner body in afterwards — and this reproduces that order exactly.
        for fixture in [
            // The two documented AsciiDoc idioms, from the language docs.
            "+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don\'t$$+.",
            "+you feel pass:q[`mono`].+",
            // Each delimited form the first pass recognizes, inside the body.
            "+a $$b$$ c+",
            "+a pass:[b] c+",
            "+a ++b++ c+",
            "+a +++<i>y</i>+++ c+",
            // The restored body carrying markup: spliced *after* the verbatim
            // substitution, so it is emitted once rather than escaped twice.
            "+a pass:[<b>x</b>] c+",
            // And a body whose own specials the substitution *does* escape,
            // beside the restored one — the order the splice has to respect.
            "+a $$<b>$$ c+",
            "+a < b $$c$$ d+",
            // The extracted construct at either edge, as the whole body, and
            // twice in one body.
            "+$$a$$+",
            "+$$a$$ b+",
            "+a $$b$$+",
            "+a $$b$$ c $$d$$ e+",
            // A STEM expression, the other node kind the one extraction pass
            // produces.
            "+a stem:[x^2] c+",
            // The escaped attribute-list bracket's own retry reaches the same
            // relaxation: the rest of the rejected match is re-scanned, and
            // the shorter bare form it finds there may itself enclose a
            // construct the first pass replaced.
            "['role']\\+++++++++This++++++++++++",
            "['role']\\+++This+++",
            "['role']\\++This++",
            "index:[attrs]+a $$b$$ c+",
            // The forms that were already at parity, unchanged.
            "+plain+",
            "+a `b` c+",
            "+a *b* c+",
            // A body carrying [`SPAN_PLACEHOLDER`] **literally**. The
            // character is an ordinary control one a source can spell, so
            // the restore's own split reads a separator where no piece stands;
            // it writes the character back rather than consuming a body that
            // is not there, which is what keeps the rest of the body from
            // being dropped. Covered at either edge, alone, and beside a real
            // restored body on both sides of it.
            "a +b\u{10}c+ d",
            "a +\u{10}+ d",
            "a +\u{10}b+ d",
            "a +b\u{10}+ d",
            "a +pass:[<b>]\u{10}tail+ d",
            "a +head\u{10}pass:[<b>]+ d",
            "a +b\u{10}c\u{10}d+ e",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlInlineRenderer {}),
                golden_passthroughs(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_bare_form_over_an_extracted_passthrough_is_one_raw_node() {
        // The shape behind that parity: one `Raw` leaf whose value already
        // carries the inner passthrough's restored body, exactly as the string
        // pipeline's own entry does by the time the restore runs.
        let nodes = build_src(Span::new("+a $$b$$ c+"));

        assert_eq!(nodes.len(), 1);
        assert_eq!(assert_raw(&nodes[0], "a b c").data(), "+a $$b$$ c+");
    }

    #[test]
    fn a_bare_unconstrained_passthrough_is_a_raw_node() {
        // `+text+` with no attribute list folds through a plain `Raw` leaf —
        // like the double-plus/double-dollar forms, an absent attrlist means
        // no stored `type_`, so the restore never wraps the text in a
        // rendered span.
        let nodes = build_src(Span::new("a +text+ b"));

        assert_eq!(nodes.len(), 3);
        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), "a text b");

        match &nodes[1] {
            InlineNode::Raw {
                value, location, ..
            } => {
                assert_eq!(value.as_ref(), "text");
                assert_eq!(location.data(), "+text+");
            }

            other => panic!("expected Raw, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
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
        // `\+text+` drops the single backslash and keeps `+text+` literal —
        // no `Raw` node — with the boundary prefix (here "see ") preserved.
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_prohibited_prefix_before_a_bare_unconstrained_form_needs_no_retry() {
        // Unlike the two attribute-list-prefixed bare forms (which need a
        // documented divergence for this — see
        // `a_prohibited_prefix_before_a_bare_attrlisted_form_retries_over_the_rest`),
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
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_passthroughs(source),
                "for {source:?}"
            );
        }
    }

    #[test]
    fn a_bare_unconstrained_match_whose_content_crosses_an_unrestorable_node_is_deferred() {
        // A candidate bare-unconstrained match whose body spans an opaque node
        // whose own fold bytes are *not* known at build time — here a `Styled`
        // span from a hand-built level — is left unrecognized rather than
        // mis-sliced. (An already-built passthrough or STEM leaf, which the
        // pass-macro level really does leave behind, is admitted and restored;
        // see `fold_matches_the_string_pipeline_for_a_bare_form_over_an_extracted_passthrough`.)
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
                attrs: crate::attributes::Attrlist::empty(source.slice(1..2).slice(0..0)),
                children: vec![],
                passthrough: None,
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
