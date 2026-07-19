//! Integration tests for cross-reference resolution (issue #461).
//!
//! These exercise the parse/resolution split: parsing records cross-references
//! as deferred, and a later pass resolves them against a complete catalog —
//! including the cross-document (Antora-style) workflow via
//! [`Parser::parse_deferred`] and [`Document::resolve_references`].
//!
//! Expected outputs were verified against Ruby Asciidoctor 2.0.

use std::collections::HashMap;

use crate::{
    Document, Parser,
    blocks::{Block, IsBlock, SimpleBlock},
    parser::{
        CatalogResolver, HtmlSubstitutionRenderer, ReferenceResolver, ResolutionContext,
        ResolvedReference, XrefSignifier,
    },
};

/// Returns the first `SimpleBlock` found in document order (recursing into
/// nested blocks).
fn first_simple<'a>(doc: &'a Document<'a>) -> &'a SimpleBlock<'a> {
    fn walk<'a>(mut blocks: impl Iterator<Item = &'a Block<'a>>) -> Option<&'a SimpleBlock<'a>> {
        blocks.find_map(|block| {
            if let Block::Simple(simple) = block {
                Some(simple)
            } else {
                walk(block.nested_blocks())
            }
        })
    }

    walk(doc.nested_blocks()).expect("expected at least one simple block")
}

/// Returns the rendered text of the first paragraph in `doc`.
fn first_paragraph<'a>(doc: &'a Document<'a>) -> &'a str {
    first_simple(doc).content().rendered()
}

/// Returns the first `SectionBlock` found in document order (recursing into
/// nested blocks).
fn first_section<'a>(doc: &'a Document<'a>) -> &'a crate::blocks::SectionBlock<'a> {
    fn walk<'a>(
        mut blocks: impl Iterator<Item = &'a Block<'a>>,
    ) -> Option<&'a crate::blocks::SectionBlock<'a>> {
        blocks.find_map(|block| {
            if let Block::Section(section) = block {
                Some(section)
            } else {
                walk(block.nested_blocks())
            }
        })
    }

    walk(doc.nested_blocks()).expect("expected at least one section block")
}

#[test]
fn forward_reference_resolves() {
    let doc = Parser::default().parse("See <<later>>.\n\n[#later]\n== Later\n");
    assert_eq!(first_paragraph(&doc), "See <a href=\"#later\">Later</a>.");
}

#[test]
fn backward_reference_resolves() {
    let doc =
        Parser::default().parse("[#first]\n== First\n\nText.\n\n== Second\n\nBack to <<first>>.\n");

    // The "Back to ..." paragraph lives in the second section.
    let mut paragraphs = vec![];
    fn collect<'a>(blocks: impl Iterator<Item = &'a Block<'a>>, out: &mut Vec<String>) {
        for block in blocks {
            if let Block::Simple(simple) = block {
                out.push(simple.content().rendered().to_string());
            }
            collect(block.nested_blocks(), out);
        }
    }
    collect(doc.nested_blocks(), &mut paragraphs);

    assert!(
        paragraphs
            .iter()
            .any(|p| p == "Back to <a href=\"#first\">First</a>."),
        "paragraphs were {paragraphs:?}"
    );
}

#[test]
fn reference_with_explicit_text() {
    let doc = Parser::default().parse("<<sec,Custom Label>>\n\n[#sec]\n== Section\n");
    assert_eq!(first_paragraph(&doc), "<a href=\"#sec\">Custom Label</a>");
}

#[test]
fn natural_reference_by_reftext() {
    let doc = Parser::default().parse("See <<The Beginning>>.\n\n== The Beginning\n");
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#_the_beginning\">The Beginning</a>."
    );
}

#[test]
fn xref_macro_form_resolves() {
    let doc = Parser::default().parse("See xref:later[].\n\n[#later]\n== Later\n");
    assert_eq!(first_paragraph(&doc), "See <a href=\"#later\">Later</a>.");
}

#[test]
fn footnote_in_heading_does_not_leak_into_xref_text() {
    // A footnote in a section title is still a real, document-order footnote,
    // but its marker must not appear in the reference text of an xref to that
    // heading. (Asciidoctor achieves this only via an explicit-ID-plus-reftext
    // workaround; the crate does it unconditionally. See issue #594.)
    let doc = Parser::default().parse(concat!(
        "See <<sect2>>.\n",
        "\n",
        "== Section 1\n",
        "\n",
        "para.footnote:[first footnote]\n",
        "\n",
        "[#sect2]\n",
        "== Section 2footnote:[second footnote]\n",
        "\n",
        "para.footnote:[third footnote]\n",
    ));

    // The xref link text is the bare title, with no footnote marker.
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#sect2\">Section 2</a>."
    );

    // The heading's footnote is nonetheless registered, numbered in document
    // order (1: first, 2: second/heading, 3: third).
    let footnotes = doc.catalog().footnotes();
    assert_eq!(
        footnotes
            .iter()
            .map(|f| (f.index.as_str(), f.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("1", "first footnote"),
            ("2", "second footnote"),
            ("3", "third footnote"),
        ]
    );
}

#[test]
fn footnote_in_heading_does_not_leak_into_generated_id() {
    // The auto-generated section ID is likewise derived from the footnote-free
    // title, so the footnote's number does not pollute the ID. A natural
    // reference (by the title's reference text) resolves to the clean ID.
    let doc = Parser::default().parse(concat!(
        "See <<Section 2>>.\n",
        "\n",
        "== Section 2footnote:[a note]\n",
    ));

    // Without the footnote-free derivation the ID would absorb the footnote
    // number (e.g. `_section_21`); it is instead the clean `_section_2`.
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#_section_2\">Section 2</a>."
    );
}

#[test]
fn footnote_reaching_a_heading_via_attribute_is_kept_out_of_xref_text() {
    // The footnote enters the title through an attribute reference, so it is not
    // visible in the raw title source. Because markers are annotated during the
    // single title render (not gated on the source text), the footnote is still
    // kept out of the reference text — and remains a real, numbered footnote.
    let doc = Parser::default().parse(concat!(
        ":disclaimer: footnote:[Not legal advice.]\n",
        "\n",
        "See <<Terms>>.\n",
        "\n",
        "== Terms{disclaimer}\n",
    ));

    assert_eq!(first_paragraph(&doc), "See <a href=\"#_terms\">Terms</a>.");

    let footnotes = doc.catalog().footnotes();
    assert_eq!(footnotes.len(), 1);
    assert_eq!(footnotes[0].index, "1");
    assert_eq!(footnotes[0].text, "Not legal advice.");
}

#[test]
fn footnote_and_xref_in_the_same_heading_render_without_sentinels() {
    // A title containing both a footnote and a cross-reference exercises the
    // deferred-template path: the footnote marker's sentinels must be stripped
    // from the deferred template too, so resolving the xref (which rebuilds the
    // rendered text from that template) does not reintroduce them.
    let doc = Parser::default().parse(concat!(
        "== Title footnote:[a note] see <<other>>\n",
        "\n",
        "[#other]\n",
        "== Other\n",
    ));

    let title = first_section(&doc).section_title();

    // No Private-Use-Area marker sentinels survive into the rendered heading.
    assert!(
        !title.contains('\u{E002}') && !title.contains('\u{E003}'),
        "heading still contains marker sentinels: {title:?}"
    );

    // The heading keeps its footnote marker and its resolved cross-reference.
    assert!(title.contains(r#"class="footnote""#), "{title:?}");
    assert!(
        title.contains(r##"<a href="#other">Other</a>"##),
        "{title:?}"
    );

    // The footnote is registered exactly once.
    assert_eq!(doc.catalog().footnotes().len(), 1);
}

#[test]
fn footnote_in_heading_does_not_advance_a_counter_twice() {
    // Deriving the footnote-free reference text from the same single render
    // means a stateful `{counter:…}` in the title advances exactly once: the
    // heading, its reference text, and the following body all agree.
    let doc = Parser::default().parse(concat!(
        "See <<Chapter 1>>.\n",
        "\n",
        "== Chapter {counter:ch}footnote:[a note]\n",
        "\n",
        "Next is {counter:ch}.\n",
    ));

    // The reference text reflects the first (and only) counter value, `1`.
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#_chapter_1\">Chapter 1</a>."
    );

    // The body's counter is `2`, not `3`: the title render did not advance it a
    // second time.
    let mut paragraphs = vec![];
    fn collect<'a>(blocks: impl Iterator<Item = &'a Block<'a>>, out: &mut Vec<String>) {
        for block in blocks {
            if let Block::Simple(simple) = block {
                out.push(simple.content().rendered().to_string());
            }
            collect(block.nested_blocks(), out);
        }
    }
    collect(doc.nested_blocks(), &mut paragraphs);

    assert!(
        paragraphs.iter().any(|p| p == "Next is 2."),
        "paragraphs were {paragraphs:?}"
    );
}

#[test]
fn unresolved_reference_falls_back_and_warns() {
    // Parse without resolving, then resolve against the document's own catalog
    // (cloned so it does not alias the `&mut doc` borrow).
    let mut doc = Parser::default().parse_deferred("See <<nope>>.\n");

    // Before resolution, the reference is pending.
    assert!(first_simple(&doc).content().has_unresolved_refs());

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].target, "nope");

    // Unresolved references still render a sensible fallback.
    assert_eq!(first_paragraph(&doc), "See <a href=\"#nope\">[nope]</a>.");
}

#[test]
fn xrefstyle_survives_deferred_resolution() {
    // `xrefstyle` formatting is compatible with the two-phase resolve mechanism
    // used for forward (and cross-document) references. The two inputs it needs
    // are resolved at their natural points: the effective *style* is a property
    // of the reference site, captured during parsing (alongside `provided_text`,
    // `window`, and `roles`), while the target's *signifier and number* live in
    // the catalog and are read only when references are resolved (alongside the
    // target's `reftext`). So a forward reference is an unresolved fallback
    // until resolution, then picks up its full styled text — the same lifecycle
    // as a plain reference.
    let src = ":sectnums:\n:xrefstyle: full\n\nSee <<install>>.\n\n\
              == One\n\n== Two\n\n=== Two-A\n\n=== Two-B\n\n\
              [#install]\n=== Installation\n";

    // Parse without resolving: the target section is parsed *after* the
    // reference, so it is still pending and renders the unresolved fallback.
    let mut doc = Parser::default().parse_deferred(src);
    assert!(first_simple(&doc).content().has_unresolved_refs());
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#install\">[install]</a>."
    );

    // Resolving against the now-complete catalog applies the full style, drawing
    // the signifier and number from the catalog entry registered for the target.
    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});
    assert!(warnings.is_empty());
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#install\">Section 2.3, &#8220;Installation&#8221;</a>.",
    );
}

#[test]
fn host_resolver_can_attach_signifier() {
    // A host resolver that builds its `href`/`text` from scratch (rather than
    // from a catalog `RefEntry`) can still opt a target into `full`/`short`
    // formatting by attaching a signifier with `with_signifier`. The style still
    // comes from the referencing document.
    let mut doc = Parser::default().parse_deferred(":xrefstyle: full\n\nSee <<install>>.\n");

    let resolver = CrossDocResolver {
        index: HashMap::from([(
            "install".to_string(),
            ResolvedReference::new(
                "guide.html#install".to_string(),
                Some("Installation".to_string()),
            )
            .with_signifier(XrefSignifier {
                label: "Section 2.3".to_string(),
                emphasize: false,
            }),
        )]),
    };

    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});
    assert!(warnings.is_empty());
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"guide.html#install\">Section 2.3, &#8220;Installation&#8221;</a>.",
    );
}

#[test]
fn reference_to_this_document_by_name_resolves_within_it() {
    // A target that names the document being parsed is a reference *into* this
    // document after all, so its fragment resolves against this document's own
    // catalog — even though the target was written in inter-document form.
    let mut doc = Parser::default()
        .with_primary_file_name("guide.adoc")
        .parse_deferred("See <<guide.adoc#install>>.\n\n[#install]\n== Installation\n");

    // The fragment names a section parsed after the reference, so it is pending
    // until resolution, exactly like a plain forward reference.
    assert!(first_simple(&doc).content().has_unresolved_refs());

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert!(warnings.is_empty());

    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#install\">Installation</a>."
    );
}

#[test]
fn host_resolver_can_override_a_derived_destination() {
    // The destination the parser derives for a target that names a document is
    // only a default: it is offered to the resolver (as
    // `ResolutionContext::derived`) rather than imposed, so a host that resolves
    // targets across a corpus can answer with its own.
    let mut doc = Parser::default().parse_deferred("See <<tigers#about,About Tigers>>.\n");

    // Until a resolver has run, the derived destination is what renders.
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"tigers.html#about\">About Tigers</a>."
    );

    let resolver = DerivedRewritingResolver;
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert!(warnings.is_empty());

    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"/en/tigers.html#about\">About Tigers</a>."
    );
}

#[test]
fn derived_destination_stands_when_the_resolver_declines() {
    // A resolver that returns `None` for a target that names another document
    // leaves the derived destination in place, and — unlike a target it could
    // not resolve — that is not reported as an unresolved reference.
    let mut doc = Parser::default().parse_deferred("See <<tigers#about>> and <<nope>>.\n");

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].target, "nope");

    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"tigers.html#about\">tigers.html</a> and <a href=\"#nope\">[nope]</a>."
    );
}

#[test]
fn escaped_reference_is_not_a_cross_reference() {
    // A backslash-escaped shorthand is emitted literally and is not deferred.
    let doc = Parser::default().parse("See \\<<later>>.\n\n[#later]\n== Later\n");
    assert!(!first_simple(&doc).content().has_unresolved_refs());
    assert_eq!(first_paragraph(&doc), "See &lt;&lt;later&gt;&gt;.");
}

/// A resolver backed by a combined, cross-document index — the shape a host
/// such as Antora would supply. The crate itself never merges catalogs.
struct CrossDocResolver {
    index: HashMap<String, ResolvedReference>,
}

impl ReferenceResolver for CrossDocResolver {
    fn resolve(&self, context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
        self.index.get(context.target).cloned()
    }
}

#[test]
fn cross_document_resolution() {
    let mut parser = Parser::default();

    // Two documents parsed independently; references left unresolved.
    let mut doc_a = parser.parse_deferred("See <<b-topic>> for details.\n");
    let doc_b = parser.parse_deferred("[#b-topic]\n== B Topic\n\nContent.\n");

    // The host builds a combined index from each document's catalog, assigning
    // its own cross-document hrefs. Building each entry with `from_entry` carries
    // the target's reftext and signifier, so cross-document `xrefstyle`
    // formatting keeps working (see `xrefstyle_carries_across_documents`).
    let mut index = HashMap::new();
    for id in ["b-topic"] {
        if let Some(entry) = doc_b.catalog().get_ref(id) {
            index.insert(
                entry.id.clone(),
                ResolvedReference::from_entry(format!("doc-b.html#{id}"), entry),
            );
        }
    }
    let resolver = CrossDocResolver { index };

    // Document A still has the pending reference until we resolve it.
    assert!(first_simple(&doc_a).content().has_unresolved_refs());

    let warnings = doc_a.resolve_references(&resolver, &HtmlSubstitutionRenderer {});
    assert!(warnings.is_empty());

    assert_eq!(
        first_paragraph(&doc_a),
        "See <a href=\"doc-b.html#b-topic\">B Topic</a> for details."
    );
    assert!(!first_simple(&doc_a).content().has_unresolved_refs());
}

#[test]
fn xrefstyle_carries_across_documents() {
    // Cross-document `xrefstyle` formatting works when the host resolver carries
    // the target's signifier. The two inputs come from different documents: the
    // *style* (`full`) is a property of the referencing document (doc A), while
    // the *signifier and number* ("Section 2.3") are computed in the target
    // document (doc B) and travel on its catalog entry. A host that builds its
    // result with `ResolvedReference::from_entry` carries the signifier through
    // automatically.
    let mut parser = Parser::default();

    let mut doc_a = parser.parse_deferred(":xrefstyle: full\n\nSee <<install>>.\n");
    let doc_b = parser.parse_deferred(
        ":sectnums:\n\n== One\n\n== Two\n\n=== Two-A\n\n=== Two-B\n\n[#install]\n=== Installation\n",
    );

    let mut index = HashMap::new();
    if let Some(entry) = doc_b.catalog().get_ref("install") {
        index.insert(
            "install".to_string(),
            ResolvedReference::from_entry("doc-b.html#install".to_string(), entry),
        );
    }
    let resolver = CrossDocResolver { index };

    let warnings = doc_a.resolve_references(&resolver, &HtmlSubstitutionRenderer {});
    assert!(warnings.is_empty());
    assert_eq!(
        first_paragraph(&doc_a),
        "See <a href=\"doc-b.html#install\">Section 2.3, &#8220;Installation&#8221;</a>.",
    );
}

#[test]
fn resolution_is_repeatable() {
    // Resolving twice against different resolvers yields the second result —
    // resolution is non-destructive.
    let mut doc = Parser::default().parse_deferred("See <<topic>>.\n");

    let first = CrossDocResolver {
        index: HashMap::from([(
            "topic".to_string(),
            ResolvedReference::new("first.html#topic".to_string(), Some("First".to_string())),
        )]),
    };
    doc.resolve_references(&first, &HtmlSubstitutionRenderer {});
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"first.html#topic\">First</a>."
    );

    let second = CrossDocResolver {
        index: HashMap::from([(
            "topic".to_string(),
            ResolvedReference::new("second.html#topic".to_string(), Some("Second".to_string())),
        )]),
    };
    doc.resolve_references(&second, &HtmlSubstitutionRenderer {});
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"second.html#topic\">Second</a>."
    );
}

#[test]
fn re_resolution_is_a_full_independent_sweep() {
    // Each call re-resolves every reference against the given resolver. A
    // resolver that no longer knows a target re-reports it as unresolved and
    // reverts the rendering to the fallback, even though an earlier pass had
    // resolved it.
    let mut doc = Parser::default().parse_deferred("See <<topic>>.\n");

    let knows_topic = CrossDocResolver {
        index: HashMap::from([(
            "topic".to_string(),
            ResolvedReference::new("first.html#topic".to_string(), Some("Topic".to_string())),
        )]),
    };
    let warnings = doc.resolve_references(&knows_topic, &HtmlSubstitutionRenderer {});
    assert!(warnings.is_empty());
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"first.html#topic\">Topic</a>."
    );

    // A second pass with an empty resolver re-reports the target and reverts to
    // the unresolved fallback.
    let knows_nothing = CrossDocResolver {
        index: HashMap::new(),
    };
    let warnings = doc.resolve_references(&knows_nothing, &HtmlSubstitutionRenderer {});
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].target, "topic");
    assert_eq!(first_paragraph(&doc), "See <a href=\"#topic\">[topic]</a>.");
}

#[test]
fn footnote_cross_references_resolve_via_host_resolver() {
    // Cross-references inside a footnote are resolved through a host-supplied
    // resolver too (the multi-document path), and an unresolved one falls back
    // and is reported.
    let mut doc =
        Parser::default().parse_deferred("Text.footnote:[See <<topic>> and <<missing>>.]\n");

    let resolver = CrossDocResolver {
        index: HashMap::from([(
            "topic".to_string(),
            ResolvedReference::new("other.html#topic".to_string(), Some("Topic".to_string())),
        )]),
    };
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert_eq!(
        doc.catalog().footnotes()[0].text,
        r##"See <a href="other.html#topic">Topic</a> and <a href="#missing">[missing]</a>."##
    );
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].target, "missing");
}

#[test]
fn footnote_debug_includes_deferred_cross_references() {
    let doc = Parser::default().parse("Text.footnote:[See <<sec>>.]\n\n[#sec]\n== Section\n");
    let footnote = &doc.catalog().footnotes()[0];
    let debug = format!("{footnote:?}");
    assert!(debug.contains("deferred"), "debug was: {debug}");
}

#[test]
fn xref_macro_honors_role_and_non_blank_window() {
    // A `role` attribute becomes a class, and a non-`_blank` window is emitted
    // as `target` without the automatic `rel="noopener"`.
    let doc = Parser::default().parse("xref:sec[Go,role=hint,window=_top]\n\n[#sec]\n== Section\n");

    assert_eq!(
        first_paragraph(&doc),
        r##"<a href="#sec" class="hint" target="_top">Go</a>"##
    );
}

#[test]
fn xrefstyle_value_interpretation() {
    // Whether `xrefstyle` is *set* matters, not just its value. An appendix
    // title is emphasized under any set style (its title is italicized rather
    // than shown verbatim), but when `xrefstyle` is unset the target's reftext
    // is used verbatim — no emphasis. This mirrors Ruby Asciidoctor, whose
    // default `xrefstyle` is nil (not `basic`).
    let with_xrefstyle = |header: &str| {
        Parser::default().parse(&format!(
            "{header}See <<data>>.\n\n[appendix]\n[#data]\n== Data\n"
        ))
    };

    // Unset: reftext verbatim, no emphasis.
    assert_eq!(
        first_paragraph(&with_xrefstyle("")),
        r##"See <a href="#data">Data</a>."##
    );

    // Explicit `basic`: the appendix title is emphasized.
    assert_eq!(
        first_paragraph(&with_xrefstyle(":xrefstyle: basic\n\n")),
        r##"See <a href="#data"><em>Data</em></a>."##
    );

    // Set but empty (`:xrefstyle:`) and any unrecognized value both behave as
    // `basic`.
    assert_eq!(
        first_paragraph(&with_xrefstyle(":xrefstyle:\n\n")),
        r##"See <a href="#data"><em>Data</em></a>."##
    );
    assert_eq!(
        first_paragraph(&with_xrefstyle(":xrefstyle: bogus\n\n")),
        r##"See <a href="#data"><em>Data</em></a>."##
    );
}

#[test]
fn xref_escapes_author_supplied_window_and_role() {
    // Author-supplied `window` and `role` values are escaped before they are
    // interpolated into HTML attributes, so a stray quote cannot break out of
    // the attribute and inject additional markup.
    let doc = Parser::default().parse(
        "xref:sec[Go,role=\"a\\\"b\",window=\"_top\\\" onclick=\\\"evil()\"]\n\n[#sec]\n== Section\n",
    );

    let rendered = first_paragraph(&doc);
    assert!(
        !rendered.contains("onclick=\"evil"),
        "attribute injection was not escaped: {rendered}"
    );
    assert!(
        rendered.contains("&quot;"),
        "expected escaped quotes in: {rendered}"
    );
}

/// A resolver that rewrites the destination the parser derived for a target
/// naming another document, the way a host with its own site layout would.
struct DerivedRewritingResolver;

impl ReferenceResolver for DerivedRewritingResolver {
    fn resolve(&self, context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
        let derived = context.derived?;

        Some(ResolvedReference::new(
            format!("/en/{href}", href = derived.href),
            Some(derived.text.clone()),
        ))
    }
}

/// Issue #772: an unresolved cross-reference is reported on the document
/// alongside every other parse-time warning, not only through the resolution
/// pass's own return value.
mod unresolved_reference_warnings {
    use crate::{
        Parser,
        parser::SafeMode,
        tests::prelude::{inline_file_handler::InlineFileHandler, *},
    };

    #[test]
    fn reported_against_the_referencing_block() {
        let doc = Parser::default().parse("== Section\n\nSee <<nope>>.\n");

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope".to_string())
        );

        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn reported_for_a_reference_inside_a_footnote() {
        // A footnote's text is lifted out of the block it was written in, but the
        // footnote records the location of its defining occurrence, so the
        // warning is anchored at that content rather than at the whole document.
        let doc = Parser::default().parse("Intro.\n\nText.footnote:[See <<nope>>.]\n");

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope".to_string())
        );

        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn distinguishes_two_footnotes_by_location() {
        // The whole point of #804: two unresolved references in two different
        // footnotes must be distinguishable by location, so a host can point the
        // author at the offending footnote.
        let doc = Parser::default()
            .parse("First.footnote:[See <<nope-a>>.]\n\nSecond.footnote:[See <<nope-b>>.]\n");

        let mut warnings: Vec<_> = doc.warnings().collect();
        warnings.sort_by_key(|w| w.source.line());
        assert_eq!(warnings.len(), 2);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope-a".to_string())
        );
        assert_eq!(warnings[0].source.line(), 1);

        assert_eq!(
            warnings[1].warning,
            WarningType::PossibleInvalidReference("nope-b".to_string())
        );
        assert_eq!(warnings[1].source.line(), 3);
    }

    #[test]
    fn reported_for_a_reference_inside_a_footnote_in_a_markdown_blockquote() {
        // A footnote defined inside a Markdown-style blockquote indexes the
        // quote's owned, `>`-stripped body, which is not contiguous in the
        // document source, so no precise location is recorded and the warning
        // falls back to the whole-document span rather than a misleading one.
        //
        // The footnote sits on a later line of the quote's owned body (offset > 0
        // there); were that owned offset stored and applied to the document
        // source it would resolve to some unrelated line, so asserting the
        // fallback line 1 guards the owned-sub-source guard specifically.
        let doc =
            Parser::default().parse("Intro.\n\n> Line one.\n>\n> Text.footnote:[See <<nope>>.]\n");

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope".to_string())
        );

        // The fallback anchor is the whole document, which begins at line 1.
        assert_eq!(warnings[0].source.line(), 1);
    }

    #[test]
    fn reported_for_a_reference_inside_a_markdown_blockquote() {
        // A Markdown-style blockquote's blocks borrow the block's own owned
        // source, so the warning is re-anchored to the blockquote in the
        // document.
        let doc = Parser::default().parse("Intro.\n\n> See <<nope>>.\n");

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope".to_string())
        );

        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn reported_for_a_reference_inside_an_include_expanded_table_cell() {
        // An include-expanded AsciiDoc table cell is parsed from its own owned
        // source, so the warning is re-anchored to the cell in the document.
        let handler = InlineFileHandler::from_pairs([("frag.adoc", "See <<nope>>.")]);

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler)
            .parse("|===\na|include::frag.adoc[]\n|===");

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            WarningType::PossibleInvalidReference("nope".to_string())
        );

        assert_eq!(warnings[0].source.line(), 2);
    }
}
