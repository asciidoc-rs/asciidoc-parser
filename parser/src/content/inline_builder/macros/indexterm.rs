//! Index-term recognition (`((term))`, `(((primary, secondary)))`,
//! `indexterm:[…]`, `indexterm2:[…]`).

use super::{LevelStrings, MacroMatch, MacroMatchKind, rebuild_macro_level, shifted_level};
// Referenced by the doc comments below, whose offset arithmetic mirrors this
// rewrite (see [`shown_term_range`]); the code performs it structurally
// rather than calling it.
#[allow(unused_imports)]
use crate::content::normalize_index_text;
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        INLINE_INDEXTERM,
        inline_builder::{
            quotes::{
                LevelContext, Piece, SPAN_PLACEHOLDER, emit_range, single_text_value, source_slice,
            },
            special_chars::Masked,
        },
    },
    inlines::{IndexTerm, InlineNode},
    strings::CowStr,
};

/// Mirrors the string step's guard: a shorthand needs a `((` … `))` pair (its
/// parens are not special, so they reach the macros step intact), and a
/// macro form needs a `:[` and `dexterm` (matching both `indexterm:` and
/// `indexterm2:`). Shared between [`indexterm_macros_level`]'s pre-build
/// sniff and its post-build one, so the two answers cannot drift apart.
fn indexterm_prefilter(s: &str) -> bool {
    (s.contains("((") && s.contains("))")) || (s.contains(":[") && s.contains("dexterm"))
}

/// Matches [`INLINE_INDEXTERM`] at this level's escaped text, replacing each
/// recognized index term — the `((term))` / `(((primary, secondary,
/// tertiary)))` shorthand and the `indexterm:[…]` / `indexterm2:[…]` macro —
/// with the [`IndexTerm`](InlineNode::IndexTerm) node it produces and leaving
/// everything else in place.
pub(super) fn indexterm_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
    level: &mut Option<LevelStrings>,
) -> Vec<InlineNode<'src>> {
    // Cheap pre-filter, taken *before* the match string is materialized: a
    // single, unsplit `Text` node's match string is its own value, so the
    // check below can run against that directly. A level already split by
    // an earlier step falls back to the build, exactly as before.
    if single_text_value(&nodes).is_some_and(|value| !indexterm_prefilter(value)) {
        return nodes;
    }

    // The level's shared shifted match string (see `shifted_level`).
    let (s, pieces) = {
        let entry = shifted_level(level, &nodes, ctx, masked);
        (entry.0.as_str(), entry.1.as_slice())
    };

    // Cheap pre-filter mirroring the string step's guard: a shorthand needs a
    // `((` … `))` pair (its parens are not special, so they reach the macros
    // step intact), and a macro form needs a `:[` and `dexterm` (matching both
    // `indexterm:` and `indexterm2:`).
    if !indexterm_prefilter(s) {
        return nodes;
    }

    let matches = find_indexterm_matches(s, &nodes, pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    // A shorthand's look-ahead retry (absorbing trailing parens) has a subtle
    // consequence: if the whole level accumulates *no* output and the last
    // match is such a retry, the level is left **unchanged** rather than
    // replaced. Concretely, content that is nothing but concealed shorthand
    // terms (`(((coffee)))`, `(((a)))(((b)))`) is left *literal*, where the
    // same terms with any surrounding output render to nothing. Detect that
    // no-op and mirror it: leave the level untouched.
    //
    // This is a **known bug** (asciidoc-rs/asciidoc-parser#1123): a
    // whole-content concealed term should render empty, not literal. The tree
    // reproduces it here to keep parity with the golden output; fixing both
    // is future work, tracked by that issue.
    if indexterm_substitution_is_a_noop(&matches) {
        return nodes;
    }

    let macro_matches = matches.into_iter().map(|m| m.macro_match).collect();

    let rebuilt = rebuild_macro_level(&nodes, pieces, s, macro_matches);
    *level = None;
    rebuilt
}

/// A recognized index-term match, plus the two facts
/// [`indexterm_substitution_is_a_noop`] needs to reproduce the whole-level
/// no-op case.
struct RecognizedIndexterm<'src> {
    macro_match: MacroMatch<'src>,

    /// Whether this match renders any output (a shown term, a kept parenthesis,
    /// or an unescaped literal). A concealed term renders nothing.
    rendered_nonempty: bool,

    /// Whether recognizing this match advanced via a look-ahead retry — true
    /// only for a shorthand that absorbed trailing parens. The macro forms
    /// never retry.
    is_skip: bool,
}

/// Reports whether this level's index-term substitution would be a **no-op**,
/// leaving the content unchanged rather than replaced (see
/// [`indexterm_macros_level`]).
///
/// That happens exactly when the accumulated output is empty *and* the last
/// recognized match advanced via a look-ahead retry: an empty gap before every
/// match, every match rendering nothing, and the final match a paren-absorbing
/// shorthand. Any non-empty gap or shown term — or a trailing macro form,
/// which never retries — means the terms are recognized normally.
fn indexterm_substitution_is_a_noop(matches: &[RecognizedIndexterm]) -> bool {
    let mut emitted_nonempty = false;
    let mut prev_end = 0;
    let mut last_is_skip = false;

    for m in matches {
        let full = &m.macro_match.full;

        // A non-empty gap before the match is literal text, making the
        // accumulated output non-empty.
        if full.start > prev_end {
            emitted_nonempty = true;
        }

        if m.rendered_nonempty {
            emitted_nonempty = true;
        }

        prev_end = full.end;
        last_is_skip = m.is_skip;
    }

    last_is_skip && !emitted_nonempty
}

/// Finds every recognized index term at this level — both the macro forms
/// (`indexterm:[…]`, `indexterm2:[…]`) and the shorthand forms (`((term))`,
/// `(((primary, secondary, tertiary)))`) — as a [`MacroMatch`].
///
/// # Concealed vs. visible, and the recognition boundary
///
/// A **concealed** term (`indexterm:[…]`, `(((…)))`) renders to *nothing* —
/// [`render_index_term`](crate::parser::InlineRenderer::render_index_term)
/// emits no output for it — so, much like an inline anchor whose output is a
/// function of its id alone (see
/// [`find_anchor_matches`](super::anchors::find_anchor_matches)), it is
/// recognized regardless of what its argument crosses; the node simply carries
/// an empty `terms`. (The one exception is the whole-level no-op for a level
/// of *only* concealed shorthand terms, which [`indexterm_macros_level`]
/// handles by leaving the level literal.)
///
/// A **visible** (flow) term (`indexterm2:[…]`, `((term))`) shows its term text
/// in the flow, so that text must be reconstructible from this level's escaped
/// match string. It is — since a [`CharRef`](InlineNode::CharRef) contributes
/// its canonical entity to the match string — whenever the term crosses no
/// *opaque span* (an
/// earlier-recognized
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref), carried
/// here as a single [`SPAN_PLACEHOLDER`] rather than its rendered markup). A
/// visible term crossing such a span is **deferred** — the match is left as
/// literal source for a later increment, pinned by a divergence test. An
/// `indexterm2:[…]` term carrying an attribute list (an `=`, whose first
/// positional attribute becomes the shown term) is *not*: the list decides
/// only which of the argument's own bytes are shown, so
/// [`shown_macro_term`] consumes it here and the node carries the same
/// already-substituted text every other visible spelling gives it.
///
/// A term crossing a [`synthesized`](Piece::synthesized) run (an attribute
/// expansion, or — reached at a tree's root — a filtered multi-line block's
/// own joined seed) is **not** deferred. The match string carries such a run's
/// bytes exactly, and this family reads its term from nowhere else — it never
/// slices `'src`, and an [`IndexTerm`] node carries no `Span`-typed field — so
/// the shown text is recovered precisely; only the node's `location` takes
/// the coarse fallback. This is the same lift the anchor and
/// bare-e-mail families already made, for the same reason.
///
/// A visible term's shown text is **not** a boundary the other macro families
/// stop at: it is ordinary flow content, and a later family must still be able
/// to recognize a construct inside it. So the shorthand spellings carry it as
/// nodes in [`children`](crate::inlines::IndexTerm::children) — always, not
/// only when it encloses a rendered span — and
/// [`apply_macro_families`](super::apply_macro_families) hands those children
/// to the families that run after this one, as their own level. The families
/// that run *before* it need nothing: their construct is already a node when
/// the term encloses it.
///
/// The **macro** spellings (`indexterm:[…]`, `indexterm2:[…]`) keep their shown
/// text as a string alone. An attribute list's shown term is the value
/// [`Attrlist::parse`](Attrlist) returns for its first positional attribute,
/// which is not a range of this level's match string, so nodes built from that
/// range would not agree with it — and this family cannot tell that case from
/// the plain one before the list is parsed. What such a term's text encloses
/// therefore stays unreached by the later families; pinned by a divergence
/// test.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect; the HTML backend generates no index, so there is nothing to record
/// here regardless.
fn find_indexterm_matches<'src>(
    s: &str,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<RecognizedIndexterm<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_INDEXTERM.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let escaped = whole.as_str().starts_with('\\');

        // Macro form: group 1 is the name (`indexterm` / `indexterm2`), group 2
        // its argument.
        if let Some(name) = caps.get(1) {
            let full = whole.start()..whole.end();
            let is_visible = name.as_str() == "indexterm2";

            // Group 2 (the macro argument) is part of the same alternation
            // branch as group 1 (`(indexterm2?):\[ (.*?[^\\]) \]`), so it is
            // always present once group 1 matched.
            #[allow(clippy::unwrap_used)]
            let arg = caps.get(2).unwrap().range();

            if let Some(m) = build_indexterm_macro_match(
                is_visible, s, arg, full, escaped, nodes, pieces, root, parser,
            ) {
                matches.push(m);
            }

            continue;
        }

        // Shorthand form: group 3 is the text enclosed by the outermost `((` …
        // `))`. Absorb any `)` immediately after the matched `))` so the
        // closing pair is the *last* in the run — a `(?!\))` look-ahead in
        // spirit, though `captures_iter`
        // cannot skip ahead, so the absorbed parens are folded into
        // this match's `full` range instead; a run of `)` never starts
        // a new match, so the next `captures_iter` match still lands
        // past them.
        //
        // The only remaining alternation branch once group 1 (checked above)
        // is absent, so group 3 (the shorthand enclosed text) is always
        // present here.
        #[allow(clippy::unwrap_used)]
        let inner = caps.get(3).unwrap().range();

        let extra = s[whole.end()..].bytes().take_while(|b| *b == b')').count();
        let full = whole.start()..(whole.end() + extra);

        // `encl_text` — the enclosed text plus any absorbed trailing parens —
        // is what classifies the shorthand as concealed vs. visible, and the
        // two runs it concatenates are *not* adjacent in the match string (the
        // matched `))` sits between them), so it is carried as the pair
        // of ranges it really is (see [`TermSource`]).
        let encl = TermSource {
            head: inner,
            tail: whole.end()..(whole.end() + extra),
        };

        push_indexterm_shorthand_matches(
            s,
            &encl,
            extra,
            full,
            escaped,
            nodes,
            pieces,
            root,
            &mut matches,
        );
    }

    matches
}

/// The source of a visible term's text, as the one or two contiguous ranges of
/// the level's match string it is spelled by.
///
/// A macro form's argument, and a shorthand whose closing `))` is the last pair
/// in its run, are one range and leave `tail` empty. A shorthand that
/// **absorbed** trailing parentheses is the case that needs two: `encl_text`
/// is the enclosed text *plus* those parens,
/// and the matched `))` sits between the two in the match string, so
/// `((coffee))))` spells `coffee))` out of `coffee` and the run of `)` that
/// follows. Every narrowing below is expressed in `encl_text`'s own
/// coordinates and mapped back onto these ranges by [`narrow`](Self::narrow),
/// so no caller has to know which case it holds.
#[derive(Clone, Debug)]
struct TermSource {
    /// The enclosed text's own range.
    head: std::ops::Range<usize>,

    /// The absorbed trailing `)` run, empty when none was absorbed.
    tail: std::ops::Range<usize>,
}

impl TermSource {
    /// The bytes this source spells — its own `encl_text`.
    fn text(&self, s: &str) -> String {
        let mut out = String::with_capacity(self.head.len() + self.tail.len());
        out.push_str(&s[self.head.clone()]);
        out.push_str(&s[self.tail.clone()]);
        out
    }

    /// Narrows to `sub`, a byte range of [`text`](Self::text)'s coordinates.
    fn narrow(&self, sub: &std::ops::Range<usize>) -> Self {
        let head_len = self.head.len();

        let head_start = sub.start.min(head_len);
        let head_end = sub.end.min(head_len);

        let tail_start = sub.start.saturating_sub(head_len).min(self.tail.len());
        let tail_end = sub.end.saturating_sub(head_len).min(self.tail.len());

        Self {
            head: (self.head.start + head_start)..(self.head.start + head_end),
            tail: (self.tail.start + tail_start)..(self.tail.start + tail_end),
        }
    }
}

/// The shown text of a visible term, in both of the forms an [`IndexTerm`]
/// node can carry it (see [`IndexTerm::children`]).
///
/// The two are built from the *same* range of the level's match string and
/// agree by construction — [`shown_term_range`] answers the normalization as
/// offsets, and [`emit_shown_term_range`] performs the two remaining byte
/// rewrites structurally — so a caller is free to take either or both.
struct ShownTerm<'src> {
    /// The already-substituted, fully computed string.
    ///
    /// `None` when the term crosses a construct whose rendering exists only at
    /// fold time, which no string built now can spell.
    computed: Option<String>,

    /// The nodes the term encloses.
    children: Vec<InlineNode<'src>>,
}

/// The range of `text` that is *shown*, as a narrowing of the
/// term's own bytes.
///
/// This is [`normalize_index_text`] and `strip_see_and_seealso` re-expressed
/// as offsets rather than as a rebuilt string, which is what lets the shown
/// text be recovered structurally (see [`shown_term`]) as well as computed. The
/// two agree by construction:
///
/// - `trim` is a narrowing already;
/// - `\n` → ` ` is length-preserving, so it moves no offset (the caller applies
///   it to the bytes it keeps);
/// - `\]` → `]` is *not*, but it never combines with the strip below — the
///   macro form unescapes brackets and does not strip, the shorthand strips and
///   does not unescape — so it perturbs nothing here and is a *gap* in what the
///   caller emits;
/// - a `see` / `see-also` clause is a suffix the strip drops, so the primary
///   term it leaves is a prefix of the normalized text and maps straight back.
fn shown_term_range(text: &str, strip_see: bool) -> std::ops::Range<usize> {
    let start = text.len() - text.trim_start().len();
    let mut end = start + text.trim_start().trim_end().len();

    if strip_see {
        // Matched over the *normalized* text, exactly as
        // [`strip_see_and_seealso`] is reached — the newline collapse can bring
        // the separator's own surrounding spaces into being.
        #[allow(clippy::indexing_slicing)]
        let normalized = text[start..end].replace('\n', " ");

        if normalized.contains(";&") {
            if let Some((primary, _see)) = normalized.split_once(" &gt;&gt; ") {
                end = start + primary.len();
            } else if let Some((primary, _see_also)) = normalized.split_once(" &amp;&gt; ") {
                end = start + primary.len();
            }
        }
    }

    start..end
}

/// The shown text of a visible term, computed as a string where the match
/// string spells every one of its bytes and recovered **structurally** where it
/// does not.
///
/// # The boundary this crosses
///
/// A visible term shows its text in the flow, so recovering it needs its
/// bytes read straight out of this level's own escaped match string. An
/// earlier-recognized construct stands in that string as one opaque
/// [`SPAN_PLACEHOLDER`], whose markup exists only when the tree is folded, so a
/// term crossing one has no string to compute from directly.
///
/// It needs none. The shown text is not a value this family *decides* anything
/// from — every decision (`trim`, the `see` clause, the attribute-list `=`) is
/// made over the bytes as matched, above — so it can be carried as the nodes it
/// encloses and rendered at fold time, which is the same move the
/// reference-bearing families made for their own display texts
/// ([`macro_text_children`](super::macro_text_children)). [`emit_range`] clones
/// each enclosed node whole, so the term carries the construct itself rather
/// than markup frozen at parse time, and a custom renderer sees it.
///
/// The two forms are exclusive and decided here: a term whose shown range holds
/// no placeholder keeps the computed string, which is every spelling that was
/// already at parity and leaves them byte-identical.
///
/// # What the normalization becomes
///
/// [`shown_term_range`] answers `trim` and the `see` / `see-also` strip as
/// offsets. The other two rewrites [`normalize_index_text`] performs are
/// emitted rather than applied: a `\n` becomes its own one-space
/// [`Text`](InlineNode::Text) node (the same collapse `normalize_index_text`
/// performs with `replace`), and — for the macro form alone — an escaped
/// closing bracket drops its backslash as a *gap* between two emitted ranges,
/// the same structural unescape
/// [`emit_range_unescaping_brackets`](super::emit_range_unescaping_brackets)
/// performs for the reference-bearing families. Neither rewrite can straddle
/// the two ranges of a [`TermSource`]: its `tail` is a run of `)` and holds
/// neither byte.
fn shown_term<'src>(
    s: &str,
    source: &TermSource,
    strip_see: bool,
    unescape_brackets: bool,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
) -> ShownTerm<'src> {
    let text = source.text(s);
    let range = shown_term_range(&text, strip_see);

    #[allow(clippy::indexing_slicing)]
    let shown = &text[range.clone()];

    let mut computed = shown.replace('\n', " ");
    if unescape_brackets {
        computed = computed.replace("\\]", "]");
    }

    let shown_source = source.narrow(&range);
    let mut children = Vec::new();

    for range in [shown_source.head, shown_source.tail] {
        emit_shown_term_range(
            s,
            range,
            unescape_brackets,
            nodes,
            pieces,
            root,
            &mut children,
        );
    }

    ShownTerm {
        computed: (!computed.contains(SPAN_PLACEHOLDER)).then_some(computed),
        children,
    }
}

/// Emits the nodes one contiguous range of a shown term covers, performing
/// [`normalize_index_text`]'s two byte rewrites structurally — see
/// [`shown_term`] for why each takes the shape it does.
fn emit_shown_term_range<'src>(
    s: &str,
    range: std::ops::Range<usize>,
    unescape_brackets: bool,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    out: &mut Vec<InlineNode<'src>>,
) {
    let mut cursor = range.start;
    let mut offset = range.start;

    while offset < range.end {
        #[allow(clippy::indexing_slicing)]
        let rest = &s[offset..range.end];

        if rest.starts_with('\n') {
            emit_range(nodes, pieces, cursor..offset, out);

            out.push(InlineNode::Text {
                value: CowStr::from(" "),
                location: source_slice(pieces, offset..(offset + 1), root),
            });

            offset += 1;
            cursor = offset;
        } else if unescape_brackets && rest.starts_with("\\]") {
            emit_range(nodes, pieces, cursor..offset, out);

            // The backslash alone is skipped; the `]` it escaped is kept, so
            // the next emitted range opens on it.
            offset += 2;
            cursor = offset - 1;
        } else {
            // `rest` is non-empty here (the loop guard `offset < range.end`
            // ensures at least one byte remains), so a first character always
            // exists. The scan advances by whole characters because
            // index-term text can contain multi-byte UTF-8 characters.
            #[allow(clippy::unwrap_used)]
            let c = rest.chars().next().unwrap();
            offset += c.len_utf8();
        }
    }

    emit_range(nodes, pieces, cursor..range.end, out);
}

/// Builds the [`MacroMatch`] for an index-term **macro** (`indexterm:[…]` /
/// `indexterm2:[…]`), or `None` when this increment defers it.
///
/// A concealed `indexterm:[…]` is always recognized (it renders nothing, so its
/// argument is never reconstructed). A visible `indexterm2:[…]`'s term is
/// normalized via [`normalize_index_text`]
/// with bracket-unescaping, and reduced through [`shown_macro_term`] when the
/// argument is an attribute list, then baked into the node's `terms` in its
/// already-substituted form — or, when it encloses a construct whose rendering
/// exists only at fold time, carried as the node's `children` instead (see
/// [`shown_term`]). Either way it is what `fold_index_term` feeds back to
/// `render_index_term` for byte-identical output.
///
/// The one spelling still deferred is an argument that is **both** an attribute
/// list and crossing such a construct: the shown term is then the list's first
/// positional attribute — a value that comes back from
/// [`Attrlist::parse`](Attrlist) rather than from a range of the match string,
/// so there is nothing to carry structurally, exactly as the other families'
/// attribute-list captures had no structural spelling of their own.
///
/// The level itself (`s`, `nodes`, `pieces`, `root`) is threaded in as the four
/// arguments every reader of a match string takes, alongside the match's own —
/// the same shape `apply_attribute_references_recursive` carries, and the same
/// reason for the allow.
#[allow(clippy::too_many_arguments)]
fn build_indexterm_macro_match<'src>(
    is_visible: bool,
    s: &str,
    arg: std::ops::Range<usize>,
    full: std::ops::Range<usize>,
    escaped: bool,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<RecognizedIndexterm<'src>> {
    // An escape (`\indexterm:…`) drops the backslash and keeps the rest
    // literal. A macro form never retries via a look-ahead, and the
    // unescaped literal is non-empty.
    if escaped {
        return Some(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            },
            rendered_nonempty: true,
            is_skip: false,
        });
    }

    let location = source_slice(pieces, full.clone(), root);

    let (node, rendered_nonempty) = if is_visible {
        let source = TermSource {
            head: arg,
            tail: 0..0,
        };

        let shown = shown_term(s, &source, false, true, nodes, pieces, root);

        let (terms, children, rendered_nonempty) = match shown.computed {
            Some(term) => {
                // Whether an attribute list narrows the shown text is decided
                // by the same byte [`shown_macro_term`] decides it by, and the
                // same one the `None` arm below tests: an argument holding no
                // `=` is not a list, so its term *is* the whole shown range.
                //
                // That is what makes the two forms separable, where this family
                // used to keep the string alone for both. With a list, the
                // shown term is the value [`Attrlist::parse`](Attrlist) returns
                // for the first positional attribute — `Coffee` out of
                // `Coffee, region=Kona` — which is not a range of this level's
                // match string, so the nodes built from that range describe
                // different text and must not be carried. Without one they
                // describe exactly the same text, so they are carried and the
                // later macro families descend into them as their own level,
                // reaching what this spelling's shown text encloses just as the
                // shorthand's does.
                let narrowed_by_a_list = term.contains('=');

                let term = shown_macro_term(term, parser);
                let rendered_nonempty = !term.is_empty();

                let children = if narrowed_by_a_list {
                    vec![]
                } else {
                    shown.children
                };

                (vec![CowStr::from(term)], children, rendered_nonempty)
            }

            None => {
                // As above, with nothing to carry as a string either: the term
                // crosses a construct whose rendering exists only at fold
                // time, and an attribute list would decide which of its bytes
                // are shown.
                if source.text(s).contains('=') {
                    return None;
                }

                (vec![], shown.children, true)
            }
        };

        (
            InlineNode::IndexTerm(IndexTerm {
                terms,
                children,
                visible: true,
                location,
            }),
            rendered_nonempty,
        )
    } else {
        // A concealed term renders nothing, so it is always recognized; its
        // argument (which never reaches the flow) is not reconstructed.
        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![],
                children: vec![],
                visible: false,
                location,
            }),
            false,
        )
    };

    Some(RecognizedIndexterm {
        macro_match: MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        },
        rendered_nonempty,
        // The macro forms always `Continue`; only the shorthand can retry.
        is_skip: false,
    })
}

/// The text an `indexterm2:[…]` shows in the flow, given its already
/// [`normalize_index_text`]-ed argument.
///
/// An argument carrying an `=` is an **attribute list**, whose first
/// *positional* attribute is the shown term (`indexterm2:[Coffee,
/// region=Kona]` shows `Coffee`); everything else it holds — a `see`, a
/// `see-also`, a `region` — names the entry in an index this crate's HTML
/// backend does not build, so it reaches the flow through nothing. Where the
/// list has no positional attribute at all (`indexterm2:[see=HTML 5]`), the
/// whole argument is shown verbatim, which is also the answer for an `=` that
/// was never an attribute list to begin with.
///
/// The argument is read from this level's escaped match string — the
/// caller has already refused an argument crossing an opaque span, so this
/// always has the same bytes the flow itself shows.
///
/// # Why the node needs no `Attrlist`
///
/// The link and cross-reference families capture the
/// [`Attrlist<'src>`](Attrlist) their own bracket parses, because a role, an
/// id, or a `window=` there changes what the fold emits. An index term's whole
/// render surface is
/// [`render_index_term`](crate::parser::InlineRenderer::render_index_term)'s
/// `visible_term`, which
/// carries the shown term and nothing else — so the list is *consumed* here
/// rather than carried, and the [`IndexTerm`] node goes on holding the same
/// already-substituted shown text every other visible spelling gives it.
fn shown_macro_term(arg: String, parser: &Parser) -> String {
    if !arg.contains('=') {
        return arg;
    }

    // Parsed over the normalized copy (`Attrlist::parse(Span::new(&arg), …)`):
    // the value read back out is owned, so nothing borrows `'src` and the
    // node keeps the coarse whole-match location a computed value falls
    // back to.
    let attrlist = Attrlist::parse(Span::new(&arg), parser, AttrlistContext::Inline)
        .item
        .item;

    let positional = attrlist.nth_attribute(1).map(|a| a.value().to_string());

    positional.unwrap_or(arg)
}

/// Pushes the [`MacroMatch`]es for an index-term **shorthand** (`((term))`,
/// `(((primary, secondary, tertiary)))`) — none at all when the visible term
/// crosses an opaque span (see [`find_indexterm_matches`]).
///
/// `inner` is the text between the outermost `((` and `))` (match group 3);
/// `extra` is the count of `)` absorbed after the closing pair. Together they
/// form `encl_text`, whose leading/trailing parentheses
/// classify the term as concealed vs. visible and carry any literal parenthesis
/// adjacent to (but not part of) the term:
///
/// - `(((x)))` → **concealed** `x` (the node consumes the whole match and
///   renders nothing);
/// - a leading extra paren → **visible** term preceded by a literal `(`;
/// - a trailing extra paren → **visible** term followed by a literal `)`;
/// - `((x))` → **visible** `x`.
///
/// A single literal parenthesis kept beside the term is expressed by pointing
/// the node's `consumed` sub-range one byte inside the match, so
/// [`rebuild_macro_level`] emits that edge `(`/`)` as literal text — the same
/// sub-range mechanism the auto-link pass uses for a bare URL's kept prefix.
///
/// # The two escaped spellings
///
/// An **escaped** shorthand (`\((…))`) drops its backslash and keeps the rest —
/// absorbed parens included — literal, an
/// [`Unescape`](MacroMatchKind::Unescape) over just the backslash.
///
/// An escaped **paren-wrapped** one (`\(((x)))`) is the exception: Asciidoctor
/// still renders it as an escaped concealed term wrapping a
/// nested flow term. It strips the wrapping parentheses off `encl_text` and
/// renders what is left as a **visible** term between two literal parens, so
/// `\(((x)))` collapses to `(x)` rather than staying literal. That is one
/// match's source doing two things, which is exactly what a **pair** of matches
/// expresses — an [`Unescape`](MacroMatchKind::Unescape) over the backslash
/// alone, then a [`Node`](MacroMatchKind::Node) over the rest with a kept paren
/// at *each* end of its `consumed` sub-range — and
/// [`rebuild_macro_level`] composes the pair as it composes any two adjacent
/// matches, so neither [`MacroMatchKind`] variant grows a new shape. It is the
/// same composition the escaped-attribute-list bracket
/// ([`passthrough_step`](super::super::passthrough_step)) reaches from the
/// other direction; only the kept-paren narrowing is this family's own, and it
/// is the narrowing the unescaped spellings above already use, applied to both
/// ends at once.
#[allow(clippy::too_many_arguments)]
fn push_indexterm_shorthand_matches<'src>(
    s: &str,
    encl: &TermSource,
    extra: usize,
    full: std::ops::Range<usize>,
    escaped: bool,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    matches: &mut Vec<RecognizedIndexterm<'src>>,
) {
    // `encl_text` = the enclosed text plus any absorbed trailing parens,
    // built before classifying.
    let encl_text = encl.text(s);

    if escaped {
        // The escaped branch's own classification: a paren-wrapped `encl_text`
        // is the nested flow term Asciidoctor still renders (see this
        // function's doc comment); anything else stays literal.
        let nested =
            (encl_text.starts_with('(') && encl_text.len() >= 2 && encl_text.ends_with(')'))
                .then(|| 1..(encl_text.len() - 1));

        let Some(nested) = nested else {
            matches.push(RecognizedIndexterm {
                macro_match: MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: full.start,
                    },
                    full,
                },
                rendered_nonempty: true,
                is_skip: extra > 0,
            });

            return;
        };

        // The backslash alone. An `Unescape` whose match *is* the character it
        // drops emits nothing.
        matches.push(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full: full.start..(full.start + 1),
            },
            rendered_nonempty: false,
            is_skip: false,
        });

        // Everything after it: the nested term, with the wrapping parenthesis
        // at each end left outside `consumed` so `rebuild_macro_level`
        // emits both as the literal text kept around the
        // rendered term.
        let term_full = (full.start + 1)..full.end;
        let consumed = (term_full.start + 1)..(term_full.end - 1);

        let shown = shown_term(s, &encl.narrow(&nested), true, false, nodes, pieces, root);

        let terms: Vec<CowStr<'src>> = shown
            .computed
            .map_or_else(Vec::new, |term| vec![CowStr::from(term)]);

        let node = InlineNode::IndexTerm(IndexTerm {
            terms,
            children: shown.children,
            visible: true,
            location: source_slice(pieces, term_full.clone(), root),
        });

        matches.push(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed,
                    node: Box::new(node),
                },
                full: term_full,
            },
            // The two kept parentheses are output whatever the term renders to.
            rendered_nonempty: true,
            is_skip: extra > 0,
        });

        return;
    }

    // Classify the term by its paren stripping.
    // `term` is the inner text whose primary term is shown (visible) or
    // indexed only (concealed); `before`/`after` flag a single literal
    // parenthesis kept beside the term.
    let len = encl_text.len();

    let (term_sub, visible, before, after): (std::ops::Range<usize>, bool, bool, bool) =
        if encl_text.starts_with('(') {
            if len >= 2 && encl_text.ends_with(')') {
                (1..(len - 1), false, false, false) // `(((concealed)))`
            } else {
                (1..len, true, true, false) // visible, kept `(`
            }
        } else if encl_text.ends_with(')') {
            (0..(len - 1), true, false, true) // visible, kept `)`
        } else {
            (0..len, true, false, false) // `((visible))`
        };

    let location = source_slice(pieces, full.clone(), root);

    let (node, rendered_nonempty) = if visible {
        let shown = shown_term(s, &encl.narrow(&term_sub), true, false, nodes, pieces, root);

        let shown_nonempty = shown.computed.as_ref().is_none_or(|term| !term.is_empty());

        let terms: Vec<CowStr<'src>> = shown
            .computed
            .map_or_else(Vec::new, |term| vec![CowStr::from(term)]);

        // The match renders output when it shows a non-empty term or keeps a
        // literal parenthesis beside it.
        let rendered_nonempty = before || after || shown_nonempty;

        (
            InlineNode::IndexTerm(IndexTerm {
                terms,
                children: shown.children,
                visible: true,
                location,
            }),
            rendered_nonempty,
        )
    } else {
        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![],
                children: vec![],
                visible: false,
                location,
            }),
            false,
        )
    };

    // A kept literal parenthesis is left outside the node's `consumed`
    // sub-range so [`rebuild_macro_level`] emits it as literal text: a
    // `before` keeps the match's first `(`; an `after` keeps its last `)`.
    let consumed = (full.start + usize::from(before))..(full.end - usize::from(after));

    matches.push(RecognizedIndexterm {
        macro_match: MacroMatch {
            kind: MacroMatchKind::Node {
                consumed,
                node: Box::new(node),
            },
            full,
        },
        rendered_nonempty,
        is_skip: extra > 0,
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{assert_text, build_src, fold_html, golden_macros};
    use crate::{
        Span,
        inlines::{IndexTerm, InlineNode},
        parser::HtmlInlineRenderer,
    };

    /// Asserts that `node` is an [`IndexTerm`](InlineNode::IndexTerm),
    /// returning it.
    fn assert_index_term<'a, 'src>(node: &'a InlineNode<'src>) -> &'a IndexTerm<'src> {
        match node {
            InlineNode::IndexTerm(index_term) => index_term,

            other => panic!("expected an IndexTerm, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_index_terms() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the golden output byte-for-byte. This is the
        // differential corpus that pins the index-term family.
        // A concealed term renders nothing, a visible term renders its shown
        // text; both spellings and the trailing-paren / see-also handling are
        // exercised.
        let fixtures = [
            // No index term despite paren/colon/bracket characters.
            "plain text without a term",
            "a single ( paren and a lone ) paren",
            "indexterm without a bracket stays literal",
            "a colon : and brackets [] but no term",
            // Macro form: concealed (no output) and visible (flow) primary.
            "indexterm:[coffee]",
            "indexterm:[coffee, robusta]",
            "indexterm2:[coffee]",
            "an indexterm2:[Coffee] in the flow",
            // Shorthand: visible `((…))` and concealed `(((…)))`.
            "((coffee))",
            "See ((coffee)) for more.",
            "(((coffee)))",
            "(((coffee, robusta, arabica)))",
            // Trailing-paren absorption: the closing pair is the *last* in a run,
            // so an extra `)` is kept as literal flow text after the term.
            "((coffee)))",
            // A leading extra paren is kept as literal flow text before the term.
            "(((coffee))",
            // A visible term with a `see` / `see-also` clause shows only its
            // primary; the `>>` / `&>` separators are `&gt;`/`&amp;` entities by
            // macro time, but the term is still reconstructed from the escaped
            // match string, so this is parity (not a divergence).
            "((Coffee >> Beans))",
            "((Coffee &> Tea))",
            // The `;&` guard hit with neither separator present: both clauses
            // need a space on *each* side, so a `>>` / `&>` that lacks the
            // trailing one is part of the shown term.
            "((Coffee >>Beans))",
            "((Coffee &>Tea))",
            // A concealed term whose inner crosses a rendered span still renders
            // nothing on both paths (the span markup is inside the discarded
            // term), so it is parity.
            "(((*bold* term)))",
            // Escapes: the term stays literal, minus the backslash.
            "\\indexterm:[x]",
            "\\indexterm2:[x]",
            "\\((coffee))",
            // A visible term crossing a *typographic replacement* — the third
            // recoverable piece, whose match-string bytes are the entity the
            // built-in backend renders it as, so the shown text is
            // reconstructed exactly, matching the golden output.
            "((Coffee (C) Beans))",
            "((O'Reilly))",
            "an indexterm2:[Tom (C) Jerry] in the flow",
            "(((Coffee (C) Beans, robusta)))",
            // Embedded in surrounding flow and next to other constructs.
            "*bold* then ((x)) here",
            "a copyright (C) then ((x))",
            "((a)) and ((b)) and indexterm:[c]",
            "((a))((b))",
            // Recognized inside a rendered span (the macros step descends).
            "*see ((x))*",
            "_indexterm2:[y] in em_",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the golden output for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_concealed_indexterm_macro_becomes_an_empty_node() {
        let nodes = build_src(Span::new("indexterm:[coffee, robusta]"));

        assert_eq!(nodes.len(), 1);
        let index_term = assert_index_term(&nodes[0]);

        // A concealed term carries no shown text and renders nothing.
        assert!(!index_term.visible);
        assert!(index_term.terms.is_empty());

        // Its location covers the whole macro.
        assert_eq!(index_term.location.data(), "indexterm:[coffee, robusta]");
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 1);
    }

    #[test]
    fn a_visible_indexterm_macro_becomes_a_flow_node() {
        let nodes = build_src(Span::new("indexterm2:[coffee]"));

        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
        assert_eq!(index_term.location.data(), "indexterm2:[coffee]");
    }

    #[test]
    fn a_visible_shorthand_becomes_a_flow_node_with_precise_span() {
        // Embedded so the node's location is not at column 1.
        let nodes = build_src(Span::new("x ((coffee)) y"));

        // `x ` then the term then ` y`.
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "x ", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");

        // The location covers the whole `((coffee))`, the delimiters included,
        // starting at column 3.
        assert_eq!(index_term.location.data(), "((coffee))");
        assert_eq!(index_term.location.col(), 3);

        assert_text(&nodes[2], " y", 1, 13);
    }

    #[test]
    fn a_concealed_shorthand_becomes_an_empty_node() {
        // Embedded after literal text so the level is not the all-concealed
        // no-op that gets left untouched (see the parity test
        // below); the concealed term is then recognized as an empty node.
        let nodes = build_src(Span::new("x (((coffee, robusta)))"));

        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "x ", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(!index_term.visible);
        assert!(index_term.terms.is_empty());
        assert_eq!(index_term.location.data(), "(((coffee, robusta)))");
        assert_eq!(index_term.location.col(), 3);
    }

    #[test]
    fn an_all_concealed_shorthand_level_stays_literal() {
        // A level whose only *output* would be from concealed shorthand terms
        // accumulates no output and ends in a look-ahead retry, so the level
        // is left literal (the #1123 bug — see
        // `indexterm_substitution_is_a_noop`). The terms are left
        // unrecognized (no `IndexTerm` node).
        //
        // Note the `(((coffee))) trailing` case: **trailing** text does *not*
        // rescue recognition, because the no-op check only looks at output
        // accumulated *up to* the retry — trailing text after it is never
        // examined. Only output
        // emitted *before* the retry (a leading/between gap, or a shown
        // term) counts;
        // see `a_concealed_term_after_leading_output_is_consumed` for that
        // contrast. (Both match the golden output exactly — verified below.)
        for source in [
            "(((coffee)))",
            "(((a)))(((b)))",
            "indexterm:[x](((y)))",
            "(((coffee))) trailing",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
                "an all-concealed level must be left literal for {source:?}: {nodes:?}"
            );

            let folded = fold_html(&nodes, &HtmlInlineRenderer {});
            assert_eq!(folded, source, "left-literal fold for {source:?}");
            assert_eq!(
                golden_macros(source),
                source,
                "golden literal for {source:?}"
            );
        }
    }

    #[test]
    fn a_concealed_term_after_leading_output_is_consumed() {
        // The contrast to the all-concealed no-op: **leading** text makes the
        // accumulated output non-empty *before* the look-ahead retry, so
        // the level is not a no-op and the concealed term is
        // recognized and consumed (renders nothing) — leaving only the
        // leading text. `leading (((coffee)))` folds to `leading `, not the
        // literal source.
        for source in ["leading (((coffee)))", "((a)) (((b)))"] {
            let folded = fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {});
            assert_eq!(
                folded,
                golden_macros(source),
                "fold diverged from the golden output for {source:?}"
            );
            assert_ne!(folded, source, "the term should be consumed for {source:?}");
        }
    }

    #[test]
    fn a_see_clause_leaves_only_the_primary_term() {
        // The `see` separator (` >> `) is `&gt;&gt;` by macro time; the term is
        // reconstructed from the escaped match string, so the node carries only
        // the primary `Coffee`, matching the golden output.
        let nodes = build_src(Span::new("((Coffee >> Beans))"));

        let index_term = assert_index_term(&nodes[0]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "Coffee");
    }

    #[test]
    fn a_kept_paren_rides_beside_the_term_as_literal_text() {
        // A leading extra paren is kept as flow text before the visible term.
        let nodes = build_src(Span::new("(((coffee))"));

        // A literal `(` then the term node.
        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "(", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
    }

    #[test]
    fn an_index_term_is_recognized_inside_a_span() {
        // The macros step descends into a `Styled` span's children, so a term
        // inside a rendered span becomes an `IndexTerm` there.
        let nodes = build_src(Span::new("*a ((b)) c*"));

        assert_eq!(nodes.len(), 1);

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert!(
                    styled
                        .children
                        .iter()
                        .any(|n| matches!(n, InlineNode::IndexTerm(_))),
                    "expected an IndexTerm inside the span, got {:?}",
                    styled.children
                );
            }

            other => panic!("expected a Styled span, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_term_enclosing_a_span() {
        // A *visible* term whose shown text encloses a rendered span (`*bold*`
        // became a `Styled` placeholder by macro time) is carried as the node's
        // `children` rather than as a computed string, so the span's markup is
        // written by the fold itself, matching the golden output byte-for-byte.
        let fixtures = [
            // The three visible spellings, alone and in flow.
            "((*bold* term))",
            "indexterm2:[*bold* term]",
            "\\(((*bold* term)))",
            "The ((*tiger*)) (Panthera tigris) is the largest cat species.",
            "an indexterm2:[*bold* term] here",
            // The span at either edge, and as the whole term.
            "((*bold*))",
            "((term *bold*))",
            "((a *b* c))",
            // Other span variants: a monospace span, a nested one, an
            // attributed one, and a smart-quote span (rendered as entities
            // rather than as a tag).
            "((`code` term))",
            "((*a _b_ c* term))",
            "(([.role]#x# term))",
            "((\"`quoted`\" term))",
            // Two spans in one term, and two terms in one level.
            "((*a* and _b_))",
            "((*a*)) and ((_b_))",
            // The normalization a shown term still performs, now expressed
            // structurally: a collapsed newline, a trimmed edge, a `see` /
            // `see-also` clause stripped off the primary, and the macro form's
            // escaped closing bracket.
            "((*a*\nb))",
            "((  *a* b  ))",
            "((*a* b >> see))",
            "((*a* b &> also))",
            "indexterm2:[*a* \\] b]",
            "indexterm2:[ *a* b ]",
            // The trailing-paren absorption, whose `encl_text` is the two
            // *non-adjacent* runs [`TermSource`] carries: a kept `)` after the
            // term, two of them, and a kept `(` before it.
            "((*a*)))",
            "((*a*))))",
            "(((*a*))",
            // A concealed term enclosing a span renders nothing on both paths,
            // and an escaped one that is not paren-wrapped stays literal.
            "(((*bold* term)))",
            "\\((*bold* term))",
            // Inside a rendered span (the macros step descends into one), and
            // beside other constructs.
            "_see ((*x* y))_",
            "a copyright (C) then ((*x* y))",
            "((*a* b)) and indexterm:[c]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the golden output for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_term_enclosing_a_span_carries_it_as_children() {
        // The shape behind the parity above: one `IndexTerm` whose `terms` is
        // empty and whose `children` hold the enclosed span itself — not the
        // markup it will fold to — so a consumer walks the construct and a
        // custom renderer writes the span.
        let nodes = build_src(Span::new("((*bold* term))"));

        assert_eq!(nodes.len(), 1);
        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert!(
            index_term.terms.is_empty(),
            "the shown text lives in `children`: {index_term:?}"
        );
        assert_eq!(index_term.location.data(), "((*bold* term))");

        match &index_term.children[..] {
            [InlineNode::Styled(styled), text] => {
                assert_eq!(styled.location.data(), "*bold*");
                assert_text(text, " term", 1, 9);
            }

            other => panic!("expected a span and a text run, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_indexterm2_attribute_lists() {
        // An `indexterm2:[…]` argument carrying an `=` splits into an
        // attribute list whose first *positional* attribute is the shown term;
        // everything else it holds names an entry in an index the HTML backend
        // does not build, so it reaches the flow through nothing. The builder
        // reads the argument from this level's escaped match string and runs
        // [`shown_macro_term`] over it.
        for fixture in [
            // The shown term is the first positional attribute.
            "indexterm2:[Coffee, region=Kona]",
            "a indexterm2:[Coffee, region=Kona] term",
            // The two golden spellings of the `see` / `see-also` named
            // attributes, which the macro form carries as attributes rather
            // than as the shorthand's ` >> ` / ` &> ` clause.
            "indexterm2:[Flash,see=HTML 5] and indexterm2:[HTML 5,see-also=\"CSS 3, SVG\"] done.",
            // No positional attribute at all: the whole argument is shown
            // verbatim, `=` included.
            "Only named indexterm2:[see=HTML 5] here.",
            "indexterm2:[region=Kona]",
            // A quoted positional attribute, whose own comma is not a
            // separator.
            "indexterm2:[\"Coffee, Kona\",region=x]",
            // An empty first positional attribute, which shows nothing.
            "x indexterm2:[,region=Kona] y",
            // A special character in either half: by macro time it is an
            // entity in both pipelines, so the attribute list is parsed from
            // the same escaped bytes on both sides.
            "indexterm2:[a < b,region=Kona]",
            "indexterm2:[Coffee,region=a < b]",
            "indexterm2:[a &copy; b,region=Kona]",
            // A typographic replacement inside the shown half — the third
            // recoverable piece, whose match-string bytes are the entity the
            // built-in backend renders it as.
            "indexterm2:[Tom (C) Jerry,region=Kona]",
            "indexterm2:[O'Reilly,see=Books]",
            // In flow beside other constructs, twice in one flow, and inside a
            // rendered span (the macros step descends into one).
            "*bold* then indexterm2:[Coffee,region=Kona] here",
            "indexterm2:[a,x=1] and indexterm2:[b,y=2]",
            "_indexterm2:[Coffee,region=Kona] in em_",
            // Spanning a newline, which `normalize_index_text` collapses to a
            // space before either side parses the list.
            "indexterm2:[Coffee,\nregion=Kona] end",
            // An escaped attribute-list form still drops its backslash and
            // stays literal.
            "\\indexterm2:[Coffee,region=Kona]",
            // The concealed spelling ignores its argument entirely, `=` or
            // not.
            "x indexterm:[Coffee,region=Kona] y",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlInlineRenderer {}),
                golden_macros(fixture),
                "fold diverged from the golden output for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_indexterm2_attribute_list_becomes_a_node_carrying_only_its_shown_term() {
        // The shape the increment asserts: the attribute list is *consumed*
        // rather than carried. An index term's whole render surface is its
        // shown term, so the node holds the same already-substituted text
        // every other visible spelling gives it — and no `Attrlist`, which is
        // what the deferral this closes had been waiting on.
        let source = "indexterm2:[Coffee, region=Kona]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "Coffee");

        // The location covers the whole macro, list included.
        assert_eq!(index_term.location.data(), source);
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 1);

        // With no positional attribute the whole argument is the shown term.
        let nodes = build_src(Span::new("indexterm2:[see=HTML 5]"));
        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "see=HTML 5");
    }

    #[test]
    fn a_real_documents_indexterm2_attribute_lists_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named
        // this increment: an `indexterm2:[…]` whose argument is an attribute
        // list must show the same term the rendered string carries — in a
        // heading's own title as well as in block content — so a tree that
        // left the macro literal would regress the moment `rendered_html()`
        // becomes a fold of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default().parse(concat!(
            "== A indexterm2:[Coffee,region=Kona] heading\n",
            "\n",
            "indexterm2:[Flash,see=HTML 5] has been supplanted by\n",
            "indexterm2:[HTML 5,see-also=\"CSS 3, SVG\"].\n",
            "\n",
            "Only named indexterm2:[see=HTML 5] here.\n",
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
                    &crate::Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected every paragraph to carry a tree");

        // The heading's own title takes the same seam (its `Title` group runs
        // the same macros step), so the shown term reaches a section title
        // too, and that title's own tree folds to the bytes it rendered.
        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let crate::blocks::Block::Section(section) = block else {
                continue;
            };

            assert_eq!(section.section_title(), "A Coffee heading");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    section.section_title_inlines(),
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }

    #[test]
    fn an_attribute_list_term_over_a_span_is_a_documented_divergence() {
        // The recognition boundary the attribute list does *not* move: a shown
        // term crossing a rendered span is unreconstructable from this level's
        // escaped match string (the span is one placeholder here, not its
        // markup), so the macro is still deferred — the `=` half is decided
        // only once the argument's own bytes are in hand.
        let source = "indexterm2:[*bold* term,region=Kona]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "a term crossing a span must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert!(folded.contains("indexterm2:["));
        assert_eq!(golden_macros(source), "<strong>bold</strong> term");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_later_family_inside_a_visible_term() {
        // A visible term's shown text is ordinary flow content, so a `link:`
        // macro, a bare URL or address, an inline anchor, or a
        // cross-reference written inside `((…))` is recognized by the pass
        // that runs after this one, exactly as if the parentheses were not
        // there. The tree reaches that by handing the term's own
        // `children` to those same passes, as their own level.
        for source in [
            "((a term with a link:t.html[T] inside)) end",
            "((a term with a mailto:t@example.org[T] inside)) end",
            "((a term with https://t.example inside)) end",
            "((a term with an [[t]] anchor inside)) end",
            "((a term with an anchor:t[Ref] inside)) end",
            // A cross-reference is not in this corpus: `golden_macros` runs a
            // bare `Content`, which has no catalog, so *every* `xref:`/`<<…>>`
            // renders to its unresolved placeholder there whether
            // or not a term encloses it. The whole-document test below drives
            // that spelling through the real parse path instead.
            // At either edge of the shown text, and as the whole of it.
            "((link:t.html[T] leads)) end",
            "((trails link:t.html[T])) end",
            "((link:t.html[T])) end",
            // Twice in one term, and two families in one term.
            "((a link:t.html[T] and link:u.html[U] term)) end",
            "((a link:t.html[T] and https://u.example term)) end",
            // Beside a twin outside the term, so the pass order that fills
            // the catalog is exercised from both sides.
            "((a link:t.html[T] term)) and link:u.html[U] here",
            "((a https://t.example term)) and https://u.example here",
            // The paren-keeping spellings, whose shown text is a narrowing of
            // the enclosed bytes rather than all of them.
            "(((a link:t.html[T] term)) end",
            "((a link:t.html[T] term))) end",
            // Inside a rendered span, and with one inside the term.
            "*((a link:t.html[T] term))* end",
            "((a *link:t.html[T]* term)) end",
            // The nested `((… ((term)) …))` shorthand this family also builds.
            "a ((((a link:t.html[T] term)))) end",
            // A term with nothing for the later families to find, unchanged.
            "((a plain term)) end",
            "((a *bold* term)) end",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
                golden_macros(source),
                "fold diverged from the golden output for {source:?}"
            );
        }
    }

    #[test]
    fn a_cross_reference_inside_a_visible_term_resolves_in_a_real_document() {
        // The one later family the byte-parity corpus above cannot reach: a
        // cross-reference needs a catalog to resolve against, which a bare
        // `Content` has none of. Driven end to end instead, where the term's
        // shown text must carry a resolved `<a href="#…">` exactly as the
        // rendered string does — in block content and in a heading's own
        // title, both of which run this same macros step.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default().parse(concat!(
            "[[target]]\n",
            "== The Target\n",
            "\n",
            "See ((a term with xref:target[the target] inside)) here.\n",
            "\n",
            "And ((a term with <<target,the target>> inside)) too.\n",
        ));

        let mut folded = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert!(
                rendered.contains(r##"<a href="#target">the target</a>"##),
                "expected a resolved cross-reference, got {rendered:?}"
            );

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded += 1;
        }

        assert_eq!(folded, 2, "expected both paragraphs to carry a tree");
    }

    #[test]
    fn a_later_family_inside_a_macro_spelling_term_is_parity_without_a_list() {
        // The half the shorthand's lift did not reach, now reached — and the
        // narrowing is what draws the line rather than the spelling.
        //
        // An `indexterm2:[…]` argument holding no `=` is not an attribute list,
        // so its shown term *is* the whole shown range and the nodes built from
        // that range describe the same text. They are carried as `children`,
        // and the families running after this one descend into them exactly as
        // they do for the shorthand.
        for source in [
            r"indexterm2:[a term with a link:t.html[T\] inside] end",
            "indexterm2:[a term with https://t.example inside] end",
        ] {
            let nodes = build_src(Span::new(source));

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_macros(source),
                "fold diverged from the golden output for {source:?}"
            );

            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::IndexTerm(_))),
                "expected the term to be recognized for {source:?}: {nodes:?}"
            );
        }
    }

    #[test]
    fn a_macro_spelling_term_narrowed_by_a_list_keeps_its_documented_divergence() {
        // The complement, and the boundary the lift above stops at. An argument
        // holding an `=` *is* an attribute list, so the shown term is what
        // [`Attrlist::parse`](Attrlist) returns for the first positional
        // attribute — `Coffee` out of `Coffee, region=Kona`. That is not a
        // range of this level's match string, so the range's nodes describe
        // different text and cannot be carried; the term stays a string and the
        // later families have nothing to descend into.
        //
        // Closing this needs the narrowing itself expressed as a range of the
        // match string, the way [`shown_term_range`] already expresses `trim`
        // and the `see` strip — its own increment.
        for source in [
            r"indexterm2:[a term with a link:t.html[T\] inside,region=Kona] end",
            "indexterm2:[a term with https://t.example inside,region=Kona] end",
        ] {
            let nodes = build_src(Span::new(source));

            assert_ne!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_macros(source),
                "expected a divergence for {source:?}"
            );

            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::IndexTerm(_))),
                "expected the term to be recognized for {source:?}: {nodes:?}"
            );
        }
    }
    #[test]
    fn an_earlier_family_inside_a_visible_terms_shown_text_is_parity() {
        // The complement of the divergence above, and the reason it is drawn
        // where it is: `image:`/`icon:` runs *before* this family, so an image
        // a visible term encloses is already an [`Image`](InlineNode::Image)
        // node when the term is built, and rides along in the term's own
        // `children`.
        for source in [
            "((a term with an image:t.png[T] inside)) end",
            "indexterm2:[a term with an image:t.png[T] inside] end",
        ] {
            let nodes = build_src(Span::new(source));

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_macros(source),
                "expected parity for {source:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_index_terms_inside_expanded_values() {
        // A term whose shown text crosses a *synthesized* run (an attribute
        // expansion) is now recognized: this family reads a term from the
        // match string alone — which carries such a run's bytes exactly — and
        // an [`IndexTerm`] node carries no `Span`-typed field, so nothing on
        // it needs an `'src` slice. The same lift the anchor and bare-e-mail
        // families already made; it closes the two divergences
        // `a_visible_term_inside_an_expanded_value_is_a_documented_divergence`
        // and its `indexterm2:` twin used to pin.
        use crate::{
            Parser,
            content::inline_builder::build,
            parser::{HtmlInlineRenderer, ModificationContext},
        };

        let parser = Parser::default()
            .with_intrinsic_attribute("term", "coffee", ModificationContext::Anywhere)
            .with_intrinsic_attribute("second", "brewing", ModificationContext::Anywhere)
            .with_intrinsic_attribute("shorthand", "((tea))", ModificationContext::Anywhere)
            .with_intrinsic_attribute("escaped", "\\(((tea)))", ModificationContext::Anywhere);

        let fixtures = [
            // The visible shorthand and macro spellings, whole and partial.
            "x (({term})) y",
            "x ((hot {term})) y",
            "x indexterm2:[{term}] y",
            "x indexterm2:[hot {term}] y",
            // The macro spelling's attribute list, whose shown term arrives
            // from an expanded value — and whose *named* half does too, so the
            // list is parsed from the synthesized run's own bytes on both
            // sides.
            "x indexterm2:[{term},region={second}] y",
            "x indexterm2:[region={second}] y",
            // The concealed spellings (always recognized, but now over an
            // expanded value too).
            "x ((({term}, {second}))) y",
            "x indexterm:[{term}, {second}] y",
            // The whole construct arriving from an expanded value.
            "x {shorthand} y",
            // A kept literal parenthesis beside an expanded term.
            "x (((({term}))) y",
            // The escaped paren-wrapped spelling, whose nested term is the same
            // shown text read from the same synthesized run — the two kept
            // parens are the level's own bytes either side of it.
            "x \\((({term}))) y",
            // The same spelling arriving *whole* from an expanded value, so the
            // two kept parens are themselves sliced out of the synthesized run
            // (as is the backslash the pair's first match drops).
            "x {escaped} y",
        ];

        for source in fixtures {
            let nodes = build(Span::new(source), &parser, None);

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    &nodes,
                    &HtmlInlineRenderer {},
                    &parser.render_context()
                ),
                crate::content::inline_builder::snapshot::recorded("indexterm_expanded", source),
                "fold diverged from the golden output for {source:?}"
            );
        }
    }

    #[test]
    fn a_term_inside_an_expanded_value_keeps_a_coarse_location() {
        // The shown term is exact — and necessarily owned, since an expanded
        // value's bytes have no `'src` counterpart — while the node's
        // `location` falls back to the whole match's source span,
        // exactly as an anchor's does.
        use crate::{
            Parser, content::inline_builder::build, parser::ModificationContext, strings::CowStr,
        };

        let parser = Parser::default().with_intrinsic_attribute(
            "term",
            "coffee",
            ModificationContext::Anywhere,
        );

        let source = "x (({term})) y";
        let nodes = build(Span::new(source), &parser, None);

        let term = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::IndexTerm(term) => Some(term),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an IndexTerm node: {nodes:?}"));

        assert!(term.visible);
        assert_eq!(term.terms.len(), 1);
        assert_eq!(term.terms[0].as_ref(), "coffee");
        assert!(matches!(term.terms[0], CowStr::Boxed(_)), "{term:?}");

        assert_eq!(term.location.data(), "(({term}))");
        assert_eq!(term.location.line(), 1);
        assert_eq!(term.location.col(), 3);
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_escaped_paren_wrapped_shorthands() {
        // The one escaped shorthand rendered rather than
        // left literal: a paren-wrapped `\(((x)))`, whose wrapping parens are
        // stripped off `encl_text` before rendering what is left as a
        // **visible** term between two literal parens ("an escaped
        // concealed term still processes a nested flow term"), so the
        // whole thing collapses to `(x)`. The builder expresses that as
        // a pair of matches — an `Unescape` over the backslash alone,
        // then a `Node` whose `consumed` sub-range keeps a paren at
        // each end — and folds to the same bytes.
        for fixture in [
            // The shape itself, alone and in flow.
            r"\(((x)))",
            r"a \(((x))) b",
            r"\(((a, b, c)))",
            // The nested term takes the *visible* branch's whole arithmetic: a
            // `see` / `see-also` clause is stripped to the primary term (the
            // separators are `&gt;&gt;` / `&amp;&gt;` entities by macro time)…
            r"\(((Coffee >> Beans)))",
            r"\(((Coffee &> Tea)))",
            // …a newline is collapsed to a space and the term is trimmed…
            "\\(((multi\nline)))",
            r"\((( spaced )))",
            // …and a special character, a restored entity, and a typographic
            // replacement all reach the shown text as the same entity bytes
            // recorded in the golden output.
            r"\(((a < b)))",
            r"\(((a &copy; b)))",
            r"\(((Tom (C) Jerry)))",
            r"\(((O'Reilly)))",
            // An empty nested term shows nothing between the kept parens.
            r"\((()))",
            r"\((( )))",
            // The trailing-paren absorption still runs first, so what is
            // wrapped can itself carry parens — the extra `)` that the closing
            // pair absorbed is the wrapper's own right half.
            r"\(((x))))",
            r"\((((x))))",
            r"\(((x)))))",
            r"\((((x)))",
            // Beside its own literal-staying twins, whose `encl_text` is not
            // paren-wrapped and which therefore only drop a backslash.
            r"\((x))\(((y)))",
            r"\indexterm:[x]\(((y)))",
            // Twice in one flow, and beside constructs the earlier steps
            // recognized.
            r"\(((x))) \(((y)))",
            r"*bold* \(((x))) _em_",
            // Recognized inside a rendered span (the macros step descends).
            r"_\(((x))) in em_",
            // The shown term makes the substitution's output non-empty, so a
            // concealed term beside it is consumed rather than left literal by
            // the `Cow::Borrowed` no-op the level would otherwise be.
            r"\(((x)))(((y)))",
            r"\(((x))) and (((y)))",
            r"(((y))) and \(((x)))",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlInlineRenderer {}),
                golden_macros(fixture),
                "fold diverged from the golden output for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_escaped_paren_wrapped_shorthand_becomes_a_term_between_two_literal_parens() {
        // The shape the pair of matches produces: the backslash is gone, the
        // wrapping parentheses ride beside the node as literal text, and the
        // node is an ordinary visible term. Its location covers the match
        // *minus* the backslash the pair's first match drops — the kept parens
        // included, as every other shorthand node's location includes its own.
        let nodes = build_src(Span::new("x \\(((coffee))) y"));

        assert_eq!(nodes.len(), 5);
        assert_text(&nodes[0], "x ", 1, 1);
        assert_text(&nodes[1], "(", 1, 4);

        let index_term = assert_index_term(&nodes[2]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
        assert_eq!(index_term.location.data(), "(((coffee)))");
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 4);

        assert_text(&nodes[3], ")", 1, 15);
        assert_text(&nodes[4], " y", 1, 16);
    }

    #[test]
    fn a_real_documents_escaped_paren_wrapped_shorthands_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named this
        // increment: an escaped paren-wrapped shorthand renders as its nested
        // term between two literal parens, so a tree that left the whole match
        // literal would regress the moment `rendered_html()` becomes a fold of
        // this tree. Driven in block content and in a heading's own title,
        // whose `Title` group runs the same macros step.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default().parse(concat!(
            "== A \\(((Coffee))) heading\n",
            "\n",
            "An escaped \\(((Coffee >> Beans))) term keeps its parens,\n",
            "and \\((()))  shows nothing between them.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert!(rendered.contains("(Coffee)"), "rendered: {rendered:?}");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 1, "expected the paragraph to carry a tree");

        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let crate::blocks::Block::Section(section) = block else {
                continue;
            };

            assert_eq!(section.section_title(), "A (Coffee) heading");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    section.section_title_inlines(),
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }

    #[test]
    fn a_real_documents_span_enclosing_terms_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named this
        // increment: a visible term enclosing a rendered span shows that span's
        // markup, so a tree that left the whole match literal would regress the
        // moment `rendered_html()` becomes a fold of this tree. Driven in block
        // content and in a heading's own title, whose `Title` group runs the
        // same macros step.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default().parse(concat!(
            "== The ((*tiger*)) heading\n",
            "\n",
            "The ((*tiger*)) (Panthera tigris) is the largest cat species,\n",
            "and an indexterm2:[_em_ term] shows its span too.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert!(
                rendered.contains("<strong>tiger</strong>"),
                "rendered: {rendered:?}"
            );

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 1, "expected the paragraph to carry a tree");

        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let crate::blocks::Block::Section(section) = block else {
                continue;
            };

            assert_eq!(
                section.section_title(),
                "The <strong>tiger</strong> heading"
            );

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    section.section_title_inlines(),
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }

    #[test]
    fn an_escaped_paren_wrapped_term_enclosing_a_span_keeps_its_shape() {
        // The nested term is a *shown* one, so it takes the same structural
        // carry every other visible spelling now does — and the pair of matches
        // this spelling is built from (an `Unescape` over the backslash, then a
        // `Node` narrowed one byte inside each end) is unchanged around it.
        let source = "\\(((*bold* term)))";
        let nodes = build_src(Span::new(source));

        // A literal `(`, the term, a literal `)` — the backslash gone.
        match &nodes[..] {
            [open, InlineNode::IndexTerm(index_term), close] => {
                assert_text(open, "(", 1, 2);
                assert_text(close, ")", 1, 18);

                assert!(index_term.visible);
                assert!(index_term.terms.is_empty());
                assert_eq!(index_term.location.data(), "(((*bold* term)))");

                match &index_term.children[..] {
                    [InlineNode::Styled(styled), text] => {
                        assert_eq!(styled.location.data(), "*bold*");
                        assert_text(text, " term", 1, 11);
                    }

                    other => panic!("expected a span and a text run, got {other:?}"),
                }
            }

            other => panic!("expected a paren-wrapped index term, got {other:?}"),
        }

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert_eq!(folded, "(<strong>bold</strong> term)");
        assert_eq!(golden_macros(source), "(<strong>bold</strong> term)");
    }
}
