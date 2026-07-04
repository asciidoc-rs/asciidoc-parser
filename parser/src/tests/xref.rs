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
        ResolvedReference,
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
    // its own cross-document hrefs.
    let mut index = HashMap::new();
    for id in ["b-topic"] {
        if let Some(entry) = doc_b.catalog().get_ref(id) {
            index.insert(
                entry.id.clone(),
                ResolvedReference {
                    href: format!("doc-b.html#{id}"),
                    text: entry.reftext.clone(),
                },
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
fn resolution_is_repeatable() {
    // Resolving twice against different resolvers yields the second result —
    // resolution is non-destructive.
    let mut doc = Parser::default().parse_deferred("See <<topic>>.\n");

    let first = CrossDocResolver {
        index: HashMap::from([(
            "topic".to_string(),
            ResolvedReference {
                href: "first.html#topic".to_string(),
                text: Some("First".to_string()),
            },
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
            ResolvedReference {
                href: "second.html#topic".to_string(),
                text: Some("Second".to_string()),
            },
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
            ResolvedReference {
                href: "first.html#topic".to_string(),
                text: Some("Topic".to_string()),
            },
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
            ResolvedReference {
                href: "other.html#topic".to_string(),
                text: Some("Topic".to_string()),
            },
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
