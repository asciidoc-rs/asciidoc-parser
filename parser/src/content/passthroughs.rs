use std::sync::LazyLock;

use regex::Regex;

use crate::{
    Parser,
    content::SubstitutionGroup,
    inlines::{InlineNode, RawOrigin},
    parser::QuoteType,
};

/// Records one inline passthrough (`+++…+++`, `++…++`, `$$…$$`, `pass:[…]`, or
/// an inline STEM macro) that the substitution pipeline extracted from a
/// block's content before running the other substitutions, and restores
/// afterward.
///
/// The collection of these for a block is observable via
/// [`Content::passthroughs`](crate::content::Content::passthroughs), analogous
/// to Asciidoctor's internal `@passthroughs` array. It exposes, for each
/// entry, the stored (unescaped) source [`text`](Self::text) and the resolved
/// [`subs`](Self::subs) that are applied to that text on restore.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Passthrough {
    pub(crate) text: String,
    pub(crate) subs: SubstitutionGroup,
}

impl Passthrough {
    /// Returns the stored, unescaped source text of this passthrough — the text
    /// that is substituted back in (after applying [`subs`](Self::subs)) when
    /// the passthrough is restored.
    ///
    /// This mirrors the `:text` of an entry in Asciidoctor's `@passthroughs`
    /// array.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the resolved substitution group applied to this passthrough's
    /// [`text`](Self::text) when it is restored, i.e. the ordered set of
    /// substitution steps in effect for it.
    ///
    /// For example, `+++…+++` resolves to [`SubstitutionGroup::None`] (no
    /// substitutions), while `++…++` and `$$…$$` resolve to
    /// [`SubstitutionGroup::Verbatim`] (special characters). This mirrors the
    /// resolved `:subs` of an entry in Asciidoctor's `@passthroughs` array.
    pub fn subs(&self) -> &SubstitutionGroup {
        &self.subs
    }
}

impl Passthrough {
    /// Builds the collection
    /// [`Content::passthroughs`](crate::content::Content::passthroughs)
    /// returns by walking the
    /// content's inline tree — the tree being authoritative for what the
    /// content *is*, exactly as it already is for what the content renders
    /// to.
    ///
    /// # Which nodes are entries
    ///
    /// Each of the seven passthrough forms records where the extraction pass
    /// makes its one entry, which is not always a leaf:
    ///
    ///   * a [`Raw`](InlineNode::Raw) node of
    ///     [`Passthrough`](RawOrigin::Passthrough) origin — `+++…+++`, `++…++`,
    ///     `$$…$$`, `pass:[…]`, and the bare `+…+` form;
    ///   * a [`Stem`](InlineNode::Stem) node, an *implicit* passthrough, plus
    ///     whatever its body's own nodes hold;
    ///   * a **marked** [`Styled`](InlineNode::Styled) span — the wrapper the
    ///     pass builds for an attribute-list-prefixed passthrough
    ///     (`[.role]++x++`, `` [x-]`x` ``). The wrapper *is* the entry, so this
    ///     records it and does **not** descend: two of the three spellings also
    ///     carry the same pair on a `Raw` leaf inside, and descending would
    ///     report them twice where the pass records once.
    ///
    /// Everything else is a container to walk through.
    ///
    /// # Order
    ///
    /// **Document order** — the tree's own — where the extraction pass returns
    /// *extraction* order. The two are not the same: the bare `+…+` form is
    /// pulled out in a second pass and STEM in a third, so
    /// `+++A+++ and stem:[B] and [x-]++C++ and ++D++` extracts as `A, C, D, B`
    /// where the author wrote `A, B, C, D`. Document order is the deliberate
    /// choice; extraction order is simply an artifact of the multi-pass
    /// extraction implementation.
    pub(crate) fn from_tree(nodes: &[InlineNode<'_>]) -> Vec<Self> {
        let mut out = vec![];
        collect_from_tree(nodes, &mut out);
        out
    }
}

/// The recursive half of [`Passthrough::from_tree`].
fn collect_from_tree(nodes: &[InlineNode<'_>], out: &mut Vec<Passthrough>) {
    for node in nodes {
        match node {
            InlineNode::Raw {
                value,
                origin: RawOrigin::Passthrough(origin),
                ..
            } => {
                out.push(Passthrough {
                    // `value` is the author's body for every form but one: a
                    // `pass:c,q[…]` body is substituted at build time, since
                    // its group is resolved there and the resulting value is
                    // what the enclosing level's `Raw` leaf carries.
                    // `source_text` is the input that produced it, and is
                    // `None` wherever the group changed nothing.
                    text: origin
                        .source_text
                        .clone()
                        .unwrap_or_else(|| value.as_ref().to_string()),
                    subs: origin.subs.clone(),
                });
            }

            InlineNode::Stem(stem) => {
                out.push(Passthrough {
                    text: stem
                        .source_text
                        .clone()
                        .unwrap_or_else(|| stem.value.as_ref().to_string()),
                    subs: stem.subs.clone(),
                });

                // A STEM expression may *embed* an already-extracted
                // passthrough (`stem:[x +++<b>+++ y]`), which the pass records
                // as an entry of its own. Those nodes are `Stem::children`.
                collect_from_tree(&stem.children, out);
            }

            InlineNode::Styled(styled) => match &styled.passthrough {
                Some(wrapper) => out.push(Passthrough {
                    text: wrapper.text.clone(),
                    subs: wrapper.subs.clone(),
                }),

                None => collect_from_tree(&styled.children, out),
            },

            InlineNode::Ref(reference) => collect_from_tree(&reference.children, out),
            InlineNode::Footnote(footnote) => collect_from_tree(&footnote.children, out),
            InlineNode::IndexTerm(index_term) => collect_from_tree(&index_term.children, out),

            _ => {}
        }
    }
}

/// Matches several variants of the passthrough inline macro, which may span
/// multiple lines.
///
/// ## Examples
///
/// * `+++text+++`
/// * `$$text$$`
/// * `pass:quotes[text]`
///
/// NOTE: We have to support an empty `pass:[]` for compatibility with
/// AsciiDoc.py.
pub(crate) static INLINE_PASS_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
        (?:
            # Optional: attrlist
            (?:
                (\\?)              # Group 1: optional backslash before [
                \[
                    ([^\[\]]+)     # Group 2: attrlist contents
                \]
            )?
            
            (\\{0,2})              # Group 3: optional escape prefix (e.g., \ or \\)

            # Passthrough span delimiters: +++, ++, or $$
            (?:
                (\+\+\+) (.*?) (\+\+\+) |   # Groups 4,5,6: triple plus
                (\+\+)   (.*?) (\+\+)   |   # Groups 7,8,9: double plus
                (\$\$)   (.*?) (\$\$)       # Groups 10,11,12: double dollar
            )

        |

            # Alternative: pass-through directive
            (\\?)                       # Group 13: optional escape before pass
            pass:
                ([a-z]+(?:,[a-z-]+)*)?  # Group 14: optional substitution step list
                \[
                     (|.*?[^\\])        # Group 15: optional content
                                        # (avoiding escape of trailing bracket)
                \]
        )"#,
    )
    .unwrap()
});

/// Matches an inline passthrough, which may span multiple lines.
///
/// ## Examples
///
/// * `+text+`
/// * `[x-]+text+`
/// * `[x-]\`text\``
///
/// NOTE: We do not support compat-mode in the Rust implementation.
pub(crate) static INLINE_PASS: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
            (?:
                                        # Option 1: [... x-] followed by `xxx`
                \b{start-half}              # Must not follow a word
                \[(x-|[^\[\]]+\ x-)\]       # Group 1: [attrlist] with x- suffix
                \`(\S(?:.*?\S)??)\`         # Group 2: `...` content

            |                           # --OR--
                                        # Option 2: [...] followed by +xxx+
                \b{start-half}              # Must not follow a word
                \[([^\[\]]+)\]              # Group 3: [attrlist]
                (\\{0,2})                   # Group 4: optional escapes
                \+(\S(?:.*?\S)??)\+         # Group 5: +...+ content (surrounded by non-space)

            |                           # --OR--
                                        # Option 3: +xxx+ without attrlist
                (?:^|([^\w;:\\]))           # Group 6: consume a preceding char, so a run
                                        # of `+` tokenizes like Asciidoctor's `gsub`
                                        # (which consumes one char before each match)
                (\\)?                       # Group 7: optional escape
                \+(\S(?:.*?\S)??)\+         # Group 8: +...+ content (surrounded by non-space)

            )

            \b{end-half}            # Must not be followed by a word character
        "#,
    )
    .unwrap()
});

/// Matches a STEM inline macro (`stem`, and its alternatives `asciimath` and
/// `latexmath`), which may span multiple lines.
///
/// ## Examples
///
/// * `stem:[x^2]`
/// * `asciimath:[x != 0]`
/// * `latexmath:[\sqrt{4} = 2]`
///
/// The content group requires at least one character whose final character is
/// not a backslash, so an empty macro (e.g. `stem:[]`) is not recognized.
pub(crate) static INLINE_STEM_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
            (\\?)                          # Group 1: optional escape
            (stem|latexmath|asciimath)     # Group 2: notation
            :
            ([a-z]+(?:,[a-z-]+)*)?         # Group 3: optional substitution list
            \[
                (.*?[^\\])                 # Group 4: expression (last char not a backslash)
            \]
        "#,
    )
    .unwrap()
});

/// Resolves the STEM notation to apply for a bare `stem` macro or block from
/// the `stem` document attribute. Any value other than `latexmath`, `latex`, or
/// `tex` (including an unset, empty, or unrecognized value) maps to AsciiMath.
pub(crate) fn stem_notation(parser: &Parser) -> QuoteType {
    match parser.attribute_value("stem").as_maybe_str() {
        Some("latexmath") | Some("latex") | Some("tex") => QuoteType::LatexMath,
        _ => QuoteType::AsciiMath,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{content::passthroughs::Passthrough, tests::prelude::*};

    #[test]
    fn inline_double_plus_with_escaped_attrlist() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"abc \[attrs]++text++"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"abc \[attrs]++text++"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "abc [attrs]text",
                },
                source: Span {
                    data: r#"abc \[attrs]++text++"#,
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
            },)
        );
    }

    #[test]
    fn content_exposes_extracted_passthrough_collection() {
        // The inline passthroughs extracted while substituting a block's
        // content are retained on the `Content` and observable afterward via
        // the public `passthroughs()` accessor, exposing each entry's stored
        // (unescaped) text and its resolved substitution list. (Not a direct
        // port of a Ruby test; it exercises the accessor surfaced for spec
        // verification.)
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("some ++<code>{code}</code>++ and +++{raw}+++ text"),
            &mut p,
        );

        let crate::blocks::Block::Simple(block) = maw.item.unwrap().item else {
            panic!("expected a simple block");
        };

        let passthroughs = block.content().passthroughs();

        assert_eq!(passthroughs.len(), 2);

        // `++…++` stores the text verbatim and applies the special-characters
        // substitution on restore.
        assert_eq!(passthroughs[0].text(), "<code>{code}</code>");
        assert_eq!(passthroughs[0].subs(), &SubstitutionGroup::Verbatim);

        // `+++…+++` stores the text verbatim and applies no substitutions.
        assert_eq!(passthroughs[1].text(), "{raw}");
        assert_eq!(passthroughs[1].subs(), &SubstitutionGroup::None);
    }

    #[test]
    fn two_entries_with_the_same_body_and_group_are_equal() {
        // What splitting the restore-only facts out actually changes for a
        // caller, and the reason the split had to happen before the tree-built
        // view: a `Passthrough` is now *exactly* its body and its group.
        //
        // These two spellings extract the same body under the same group and
        // differ only in the attribute list the restore pass re-renders the
        // result inside. They used to compare **unequal** — `Passthrough`
        // derives `PartialEq` over its fields, and the prefixed one carried
        // `type_: Some(Unquoted)` and `attrlist: Some("role")` where the bare
        // one carried `None`. A tree-built view cannot reproduce either fact
        // (the wrapper node holds a *parsed* attrlist built from the
        // substituted source, not the author's bytes), so leaving them on the
        // public type would have made equality depend on which side built the
        // entry.
        let entry = |source: &str| -> Passthrough {
            let mut p = Parser::default();
            let maw = crate::blocks::Block::parse(crate::Span::new(source), &mut p);

            let crate::blocks::Block::Simple(block) = maw.item.unwrap().item else {
                panic!("expected a simple block");
            };

            let passthroughs = block.content().passthroughs();
            assert_eq!(passthroughs.len(), 1, "{source:?}");

            passthroughs[0].clone()
        };

        let bare = entry("a ++dup++ x");
        let prefixed = entry("a [.role]++dup++ x");

        assert_eq!(bare.text(), "dup");
        assert_eq!(bare, prefixed);

        // Still unequal where the *documented* facts differ, so the assertion
        // above is about the fields that left rather than about `PartialEq`
        // having stopped discriminating.
        assert_ne!(bare, entry("a +++dup+++ x"));
        assert_ne!(bare, entry("a ++other++ x"));
    }

    #[test]
    fn passthrough_attrlist_drop_line_does_not_leak_a_mislocated_warning() {
        // A missing reference in a passthrough's stored attribute list is
        // substituted against temporary (owned) text, so any warning it records
        // carries an offset into that text rather than the document source.
        // Such warnings must be discarded (the offset cannot be mapped back),
        // not surfaced against the document root. Regression guard for the
        // `drop-line`/`warn` attrlist path.
        let mut p = Parser::default().with_intrinsic_attribute(
            "attribute-missing",
            "drop-line",
            ModificationContext::ApiOnly,
        );

        let doc = p.parse("['{missing}']++x++");

        assert_eq!(doc.warnings().count(), 0);
    }

    #[test]
    fn passthrough_body_warnings_survive_the_builds_own_discard() {
        // The complement of the test above, and the two together are the whole
        // of the distinction.
        //
        // A passthrough carrying its own substitution list has its body
        // rendered by `passthrough_text`, which builds the body's own tree and
        // folds it. A reference the body carries is therefore recognized by
        // that build, and its `attribute-missing` diagnostic is recorded the
        // way every build records one — through `record_builder_diagnostic`,
        // which the enclosing seam drains and carries across.
        //
        // The distinction this pins is against the test above: the enclosing
        // build discards everything it records into the *substitution-warning*
        // buffer, because what lands there during a build is incidental (the
        // mislocated `Attrlist` warning). A body's own diagnostics are not
        // incidental, and they survive because they never go into that buffer
        // in the first place.
        //
        // Before the authoritative-pass closure the route was different — the
        // body re-entered `SubstitutionGroup::apply`, which raised the
        // warning into the discarded range, and a
        // `nested_authoritative_warnings` buffer on the `Parser` carried it
        // back out. That buffer is retired along with the re-entry it existed
        // for. Both routes surfaced the same warning at the same location,
        // which is what this test has always asserted and still does.
        for (source, mode) in [
            ("pass:a[{missing}]", "warn"),
            ("pass:a[{missing}]", "drop-line"),
            ("pass:q,a[*{missing}*]", "warn"),
            ("pass:c,a[{missing}]", "warn"),
        ] {
            let mut parser = Parser::default().with_intrinsic_attribute(
                "attribute-missing",
                mode,
                ModificationContext::Anywhere,
            );

            let doc = parser.parse(source);

            // The whole list, not a filtered count: these fixtures raise
            // exactly this one warning, so comparing the list also says nothing
            // *else* was surfaced — and leaves the test with no branch of its
            // own that never runs.
            let warnings: Vec<_> = doc
                .warnings()
                .map(|warning| warning.warning.clone())
                .collect();

            assert_eq!(
                warnings,
                [WarningType::SkippingReferenceToMissingAttribute(
                    "missing".to_string()
                )],
                "the body's own missing-reference warning was lost for {source:?} under {mode}"
            );
        }
    }

    #[test]
    fn a_rescued_passthrough_warning_points_at_the_reference_itself() {
        // The rescue above carries a warning across the build's discard; this
        // pins *where* the warning it carries points.
        //
        // It used to point at offset 0 — anywhere the passthrough sat.
        // `passthrough_text` seeded the body's `Content` from an unanchored
        // `Span::new`, so a reference inside the body had no position in the
        // document to be located against and every such warning collapsed onto
        // the document start. `origin/inline-ast` before this branch's
        // authoritative-pass inversion was mislocated too, differently: the
        // body's warning came from the *builder* rather than from this
        // authoritative string pass,
        // and reported the reference's offset within the body (2 for
        // `['{alpha}'`) read as though it were a document offset.
        //
        // Neither number pointed at the reference, so restoring the older one
        // was not on the table; the body is now substituted against its own
        // source span and the warning lands on the reference, exactly as the
        // plain non-passthrough control below always has. That control is what
        // says the two paths agree rather than merely both moving.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "attribute-missing",
            "warn",
            ModificationContext::Anywhere,
        );

        let doc = parser.parse("Intro.\n\nA later para with pass:a[{missing}] in it.");

        let located: Vec<_> = doc
            .warnings()
            .map(|warning| (warning.source.line(), warning.source.byte_offset()))
            .collect();

        assert_eq!(
            located,
            [(3, 33)],
            "a passthrough body's warning must point at the reference in the body"
        );

        let mut parser = Parser::default().with_intrinsic_attribute(
            "attribute-missing",
            "warn",
            ModificationContext::Anywhere,
        );

        let doc = parser.parse("Intro.\n\nA later para with {missing} in it.");

        let located: Vec<_> = doc
            .warnings()
            .map(|warning| (warning.source.line(), warning.source.byte_offset()))
            .collect();

        assert_eq!(
            located,
            [(3, 26)],
            "a plain missing reference must still be located exactly"
        );
    }

    #[test]
    fn a_nested_attributed_passthrough_locates_each_reference_separately() {
        // The shape nothing covered, and the gap the authoritative-pass
        // inversion's own offset shift went unnoticed through: a `pass:`
        // macro whose list
        // includes `a`, whose body is itself an attribute-listed inline
        // passthrough. The macro's body stops at the first `]`, so the body
        // substituted here is `['{alpha}'` — the inner passthrough's attribute
        // list, reached through the body path rather than through
        // `PassthroughRestoreReplacer`'s own stored-attrlist path.
        //
        // Two references, one inside that body and one after it, so the
        // assertion says each is located on its own rather than that some
        // single offset happens to be right. Byte offsets rather than a
        // matched substring because the whole point is the position: 11 is
        // `{alpha}` and 30 is `{beta}` in the source below.
        for mode in ["warn", "drop-line"] {
            let mut parser = Parser::default().with_intrinsic_attribute(
                "attribute-missing",
                mode,
                ModificationContext::Anywhere,
            );

            let doc = parser.parse("pass:m,a[['{alpha}']++x++ and {beta}]");

            let located: Vec<_> = doc
                .warnings()
                .map(|warning| (warning.warning.clone(), warning.source.byte_offset()))
                .collect();

            assert_eq!(
                located,
                [
                    (
                        WarningType::SkippingReferenceToMissingAttribute("alpha".to_string()),
                        11
                    ),
                    (
                        WarningType::SkippingReferenceToMissingAttribute("beta".to_string()),
                        30
                    ),
                ],
                "each reference must be located where it is written, under {mode}"
            );
        }
    }

    #[test]
    fn content_without_passthroughs_exposes_an_empty_collection() {
        // Plain content — and content whose substitution group never extracts
        // passthroughs — exposes an empty collection rather than a
        // placeholder value.
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(crate::Span::new("just plain prose"), &mut p);

        let crate::blocks::Block::Simple(block) = maw.item.unwrap().item else {
            panic!("expected a simple block");
        };

        assert!(block.content().passthroughs().is_empty());
    }
}
