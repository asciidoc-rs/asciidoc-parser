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
fn block_anchor_reftext_substitutes_attributes() {
    // A `[[id,reftext]]` block anchor may carry attribute references in its
    // reftext. They are resolved before the reftext is registered, so a natural
    // cross reference matches — and renders — the substituted text.
    let doc = Parser::default().parse(concat!(
        ":os: Linux\n",
        "\n",
        "See <<Notes for Linux>>.\n",
        "\n",
        "[[notes,Notes for {os}]]\n",
        "Some notes.\n",
    ));
    assert_eq!(
        first_paragraph(&doc),
        "See <a href=\"#notes\">Notes for Linux</a>."
    );
    assert_eq!(
        doc.catalog().get_ref("notes").unwrap().reftext.as_deref(),
        Some("Notes for Linux")
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

/// Issue #808: an inter-document cross reference whose target names a file that
/// was included into this document collapses to a same-document reference — but
/// only when the file was included in full.
mod included_file_collapses_to_internal_anchor {
    use super::first_paragraph;
    use crate::{Parser, parser::SafeMode, tests::prelude::inline_file_handler::InlineFileHandler};

    /// `other-chapters.adoc`, defining the `ch2` section. It carries a `ch2`
    /// tagged region (so a partial include can select a portion of it) and a
    /// separate `ch2-noid` region (for the fully-and-partially-included case).
    const OTHER_CHAPTERS: &str = "[#ch2]\n== Chapter 2\n\n// tag::ch2[]\nThe second chapter.\n// end::ch2[]\n\n// tag::ch2-noid[]\nAn extra note.\n// end::ch2-noid[]\n";

    fn parse_book(body: &str) -> crate::Document<'static> {
        let handler = InlineFileHandler::from_pairs([("other-chapters.adoc", OTHER_CHAPTERS)]);
        let source = format!("= Book Title\n:doctype: book\n\n{body}");
        Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler)
            .parse(&source)
    }

    #[test]
    fn full_include_collapses_reference_with_fragment() {
        // The whole file is included, so `ch2` is now an anchor in this
        // document: `<<other-chapters.adoc#ch2>>` resolves to `#ch2`.
        let doc = parse_book(
            "Read <<other-chapters.adoc#ch2>> next!\n\ninclude::other-chapters.adoc[]\n",
        );

        assert!(doc.catalog().was_included("other-chapters"));
        assert!(doc.catalog().include_is_full("other-chapters"));

        assert_eq!(
            first_paragraph(&doc),
            r##"Read <a href="#ch2">Chapter 2</a> next!"##
        );
    }

    #[test]
    fn full_include_via_double_star_tag_collapses_reference() {
        // `tags=**` selects the entire file, so the include is full.
        let doc = parse_book(
            "Read <<other-chapters.adoc#ch2>> next!\n\ninclude::other-chapters.adoc[tags=**]\n",
        );

        assert!(doc.catalog().include_is_full("other-chapters"));

        assert_eq!(
            first_paragraph(&doc),
            r##"Read <a href="#ch2">Chapter 2</a> next!"##
        );
    }

    #[test]
    fn partial_include_does_not_collapse_reference() {
        // Only the `ch2` region is included, so the reference may point at an
        // anchor that never made it across: it stays an inter-document
        // reference and keeps its derived output path.
        let doc = parse_book(
            "Read <<other-chapters.adoc#ch2,the next chapter>> next!\n\ninclude::other-chapters.adoc[tags=ch2]\n",
        );

        assert!(doc.catalog().was_included("other-chapters"));
        assert!(!doc.catalog().include_is_full("other-chapters"));

        assert_eq!(
            first_paragraph(&doc),
            r#"Read <a href="other-chapters.html#ch2">the next chapter</a> next!"#
        );
    }

    #[test]
    fn full_and_partial_includes_collapse_reference() {
        // The file is included once in full and once partially; the full
        // include wins, so the reference collapses.
        let doc = parse_book(
            "Read <<other-chapters.adoc#ch2,the next chapter>> next!\n\ninclude::other-chapters.adoc[]\n\ninclude::other-chapters.adoc[tags=ch2-noid]\n",
        );

        assert!(doc.catalog().include_is_full("other-chapters"));

        assert_eq!(
            first_paragraph(&doc),
            r##"Read <a href="#ch2">the next chapter</a> next!"##
        );
    }

    #[test]
    fn include_outside_base_directory_still_registers_and_collapses() {
        // A file included from outside the base directory registers under the
        // target as written (`../section-a`), and an inter-document reference to
        // it collapses just the same.
        let handler =
            InlineFileHandler::from_pairs([("../section-a.adoc", "[#section-a]\n== Section A\n")]);
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler)
            .parse("= Document Title\n\nSee <<../section-a.adoc#section-a>>.\n\ninclude::../section-a.adoc[]\n");

        assert!(doc.catalog().was_included("../section-a"));
        assert!(doc.catalog().include_is_full("../section-a"));

        assert_eq!(
            first_paragraph(&doc),
            r##"See <a href="#section-a">Section A</a>."##
        );
    }

    #[test]
    fn a_reference_to_a_file_that_was_not_included_is_unaffected() {
        // With no include of `other-chapters.adoc`, the reference keeps its
        // inter-document output path.
        let doc = parse_book("Read <<other-chapters.adoc#ch2,the next chapter>> next!\n");

        assert!(!doc.catalog().was_included("other-chapters"));

        assert_eq!(
            first_paragraph(&doc),
            r#"Read <a href="other-chapters.html#ch2">the next chapter</a> next!"#
        );
    }

    #[test]
    fn a_nested_include_does_not_register_and_cannot_falsely_collapse() {
        // The root document includes `part1/chapters.adoc`, which itself
        // includes `intro.adoc` — a target relative to *that* file, i.e.
        // `part1/intro.adoc` on disk. Registering it under the key `intro`
        // would collide with the root-relative path of a *different* file, so
        // a root-level `<<intro.adoc#…>>` would falsely collapse to an
        // internal anchor. A nested include is therefore not registered at
        // all, and the reference keeps its inter-document output path.
        let handler = InlineFileHandler::from_pairs([
            (
                "part1/chapters.adoc",
                "== Chapters\n\ninclude::intro.adoc[]\n",
            ),
            ("intro.adoc", "[#nested-intro]\n== Nested Intro\n"),
        ]);

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler)
            .parse("See <<intro.adoc#other,text>>.\n\ninclude::part1/chapters.adoc[]\n");

        // The outermost include registers; the nested one does not.
        assert!(doc.catalog().was_included("part1/chapters"));
        assert!(!doc.catalog().was_included("intro"));

        assert_eq!(
            first_paragraph(&doc),
            r#"See <a href="intro.html#other">text</a>."#
        );
    }

    #[test]
    fn an_include_inside_a_table_cell_registers_and_collapses() {
        // An AsciiDoc table cell shares the enclosing document's catalog, so a
        // file the cell includes in full registers on the document's include
        // registry (as in Asciidoctor, where the cell's nested document shares
        // the parent's `catalog[:includes]`) and an inter-document reference in
        // that cell collapses to an internal anchor.
        let handler =
            InlineFileHandler::from_pairs([("chapter.adoc", "[#target]\n== Chapter\n\nBody.\n")]);

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler)
            .parse("|===\na|See xref:chapter.adoc#target[].\n\ninclude::chapter.adoc[]\n|===\n");

        assert!(doc.catalog().include_is_full("chapter"));

        // The cell's blocks live in the cell's own owned sub-document, so the
        // rendered link is asserted through the converted output.
        crate::tests::assert_dom::assert_xpath(&doc, r##"//a[@href="#target"]"##, 1);
        crate::tests::assert_dom::assert_xpath(
            &doc,
            r##"//a[@href="#target"][text()="Chapter"]"##,
            1,
        );
    }
}

/// Cross-references embedded in section headings and block titles resolve to
/// the target's reference text via the document-order title resolution pass
/// (issue #770). These exercise the pass across the block types that carry a
/// title, plus the corners of how a title maps (or does not map) to a
/// referenceable target.
mod xrefs_in_titles {
    use crate::{Parser, blocks::IsBlock};

    /// Collects `(context, title)` for every titled block in the document, in
    /// document order.
    fn titled_blocks(doc: &crate::Document<'_>) -> Vec<(String, String)> {
        fn walk(block: &crate::blocks::Block<'_>, out: &mut Vec<(String, String)>) {
            if let Some(title) = block.title() {
                out.push((block.raw_context().to_string(), title.to_string()));
            }
            for child in block.nested_blocks() {
                walk(child, out);
            }
        }

        let mut out = vec![];
        for block in doc.nested_blocks() {
            walk(block, &mut out);
        }
        out
    }

    #[test]
    fn resolves_xref_in_title_of_every_titled_block_type() {
        // One document exercising an xref title on each block type that can
        // carry a `.Title`: paragraph, image (media), list, listing (raw
        // delimited), example (compound delimited), admonition, quote, table,
        // and thematic break. Every title resolves to the target section's
        // reference text.
        // The final paragraph's title has no cross-reference; it must pass
        // through the resolution pass untouched.
        let doc = Parser::default().parse(
            ".<<goal>>\npara\n\n\
             .<<goal>>\nimage::a.png[]\n\n\
             .<<goal>>\n* item\n\n\
             .<<goal>>\n----\ncode\n----\n\n\
             .<<goal>>\n====\nexample\n====\n\n\
             .<<goal>>\nNOTE: note\n\n\
             .<<goal>>\n[quote]\nquoted\n\n\
             .<<goal>>\n|===\n|cell\n|===\n\n\
             .<<goal>>\n'''\n\n\
             .Plain title\nplain para\n\n\
             [#goal]\n== Goal",
        );

        let mut titles = titled_blocks(&doc);

        // The xref-free title is untouched by the pass; the remaining titles
        // all resolve.
        let plain_position = titles
            .iter()
            .position(|(_, title)| title == "Plain title")
            .expect("expected the xref-free title to be present and untouched");
        titles.remove(plain_position);

        let resolved = r##"<a href="#goal">Goal</a>"##;

        let expected_contexts = [
            "paragraph",
            "image",
            "list",
            "listing",
            "example",
            "admonition",
            "quote",
            "thematic_break",
        ];

        // The table's context is checked separately below; contexts here are
        // asserted as a set because the exact list is not the point — each
        // titled block resolving its title is.
        assert_eq!(titles.len(), 9, "titles were: {titles:?}");

        for (context, title) in &titles {
            assert_eq!(
                title, resolved,
                "block with context {context} did not resolve its title"
            );
        }

        for context in expected_contexts {
            assert!(
                titles.iter().any(|(c, _)| c == context),
                "no titled block with context {context}; titles were: {titles:?}"
            );
        }

        assert!(
            titles.iter().any(|(c, _)| c == "table"),
            "no titled table; titles were: {titles:?}"
        );
    }

    #[test]
    fn section_with_explicit_reftext_still_resolves_its_own_title_xref() {
        // Section `a` carries an explicit `reftext`, so a reference *to* it
        // renders that reftext — not its (xref-bearing) title. The xref inside
        // `a`'s own title still resolves normally.
        let doc = Parser::default()
            .parse("[reftext=Custom Text]\n[#a]\n== A <<b>>\n\n[#b]\n== B\n\npara with <<a>>");

        let sections: Vec<_> = doc
            .nested_blocks()
            .filter_map(|block| {
                if let crate::blocks::Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(sections[0].section_title(), r##"A <a href="#b">B</a>"##);

        // The paragraph reference to `a` uses the explicit reftext.
        let para = super::first_paragraph(&doc);
        assert_eq!(para, r##"para with <a href="#a">Custom Text</a>"##);
    }

    #[test]
    fn section_with_double_bracket_anchor_is_a_recomputable_target() {
        // The section's ID comes from a `[[a]]` block anchor (rather than a
        // `[#a]` attribute), and a forward reference to it from another title
        // renders its resolved title.
        let doc = Parser::default().parse("== See <<a>>\n\n[[a]]\n== A <<b>>\n\n[#b]\n== B");

        let sections: Vec<_> = doc
            .nested_blocks()
            .filter_map(|block| {
                if let crate::blocks::Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            })
            .collect();

        // `a`'s resolved title (`A <a href="#b">B</a>`) is spliced into the
        // first heading's reference with the nested anchor dropped.
        assert_eq!(sections[0].section_title(), r##"See <a href="#a">A B</a>"##);
        assert_eq!(sections[1].section_title(), r##"A <a href="#b">B</a>"##);
    }

    #[test]
    fn title_carried_across_a_section_heading_still_resolves_its_xref() {
        // A `.Title` above a section heading does not become the section's
        // title; it is carried to the section's first child block. The carried
        // title keeps its deferred cross-reference, so the forward reference
        // still resolves once `goal` is defined.
        let doc = Parser::default().parse(".See <<goal>>\n== Section\n\npara\n\n[#goal]\n== Goal");

        let section = crate::tests::prelude::first_section(&doc);
        let para = section
            .nested_blocks()
            .next()
            .expect("expected the section's first child block");

        assert_eq!(para.title(), Some(r##"See <a href="#goal">Goal</a>"##));
    }

    #[test]
    fn resolver_chosen_text_wins_over_the_local_title() {
        // A caller-supplied resolver may resolve a target to its local `#id`
        // destination while choosing its own display text (e.g. a curated
        // index label). The resolver's choice wins; the locally computed title
        // only replaces text that echoes the catalog's frozen reference text.
        struct BespokeResolver;

        impl crate::parser::ReferenceResolver for BespokeResolver {
            fn resolve(
                &self,
                context: &crate::parser::ResolutionContext<'_>,
            ) -> Option<crate::parser::ResolvedReference> {
                (context.target == "b").then(|| {
                    crate::parser::ResolvedReference::new(
                        "#b".to_string(),
                        Some("Bespoke Text".to_string()),
                    )
                })
            }
        }

        let mut doc = Parser::default().parse_deferred("== See <<b>>\n\n[#b]\n== B <<missing>>");

        doc.resolve_references(
            &BespokeResolver,
            &crate::parser::HtmlSubstitutionRenderer {},
        );

        let sections: Vec<_> = doc
            .nested_blocks()
            .filter_map(|block| {
                if let crate::blocks::Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            sections[0].section_title(),
            r##"See <a href="#b">Bespoke Text</a>"##
        );
    }

    #[test]
    fn a_target_the_resolver_does_not_know_stays_unresolved_in_a_title() {
        // When a caller-supplied resolver cannot resolve a target, the title's
        // reference stays a bracketed fallback and is reported — even though
        // this document's own catalog knows the ID. The resolver is
        // authoritative, matching how body content is resolved.
        struct KnowsNothing;

        impl crate::parser::ReferenceResolver for KnowsNothing {
            fn resolve(
                &self,
                _context: &crate::parser::ResolutionContext<'_>,
            ) -> Option<crate::parser::ResolvedReference> {
                None
            }
        }

        let mut doc = Parser::default().parse_deferred("== See <<b>>\n\n[#b]\n== B <<c>>");

        let warnings =
            doc.resolve_references(&KnowsNothing, &crate::parser::HtmlSubstitutionRenderer {});

        let sections: Vec<_> = doc
            .nested_blocks()
            .filter_map(|block| {
                if let crate::blocks::Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(sections[0].section_title(), r##"See <a href="#b">[b]</a>"##);

        // Both title references (`b` and `c`) are reported unresolved.
        let mut targets: Vec<_> = warnings.iter().map(|w| w.target.as_str()).collect();
        targets.sort_unstable();
        assert_eq!(targets, vec!["b", "c"]);
    }

    #[test]
    fn external_href_with_coinciding_text_is_not_given_the_local_title() {
        // A resolver may map a target to an *external* destination while
        // choosing display text that happens to equal this document's frozen
        // reference text for the same ID. Resolver ownership cannot be
        // inferred from text equality; the local title applies only when the
        // reference's destination is the local target itself, so the external
        // reference is kept exactly as the resolver returned it.
        struct ExternalResolver;

        impl crate::parser::ReferenceResolver for ExternalResolver {
            fn resolve(
                &self,
                context: &crate::parser::ResolutionContext<'_>,
            ) -> Option<crate::parser::ResolvedReference> {
                match context.target {
                    // The external destination, with text that coincides with
                    // this document's frozen (parse-time) reftext for `b`.
                    "b" => Some(crate::parser::ResolvedReference::new(
                        "other.html#b".to_string(),
                        Some(r##"B <a href="#c">[c]</a>"##.to_string()),
                    )),
                    "c" => Some(crate::parser::ResolvedReference::new(
                        "#c".to_string(),
                        Some("C".to_string()),
                    )),
                    _ => None,
                }
            }
        }

        let mut doc =
            Parser::default().parse_deferred("== See <<b>>\n\n[#b]\n== B <<c>>\n\n[#c]\n== C");

        doc.resolve_references(
            &ExternalResolver,
            &crate::parser::HtmlSubstitutionRenderer {},
        );

        let sections: Vec<_> = doc
            .nested_blocks()
            .filter_map(|block| {
                if let crate::blocks::Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            })
            .collect();

        // The resolver's text is kept (nested anchors dropped at render time);
        // had the local title been spliced in, the link text would instead be
        // the freshly computed `B C`.
        assert_eq!(
            sections[0].section_title(),
            r##"See <a href="other.html#b">B [c]</a>"##
        );
    }
}
