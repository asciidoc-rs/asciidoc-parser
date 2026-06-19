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
