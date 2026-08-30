//! Realistic-density parse throughput, weighted toward the inline
//! substitution passes.
//!
//! The other benches in this directory parse tiny, nearly markup-free
//! snippets, so they hold the block parser and the per-parse fixed costs
//! steady but exercise almost none of the inline machinery: no quoted-text
//! construct is ever recognized, no macro target resolved, no character
//! replaced. A regression confined to that machinery — the code that sweeps
//! every paragraph with the quote, replacement, and macro rules — can pass
//! them untouched while multiplying the cost of converting a real document.
//!
//! Each benchmark here parses a document of a few hundred lines whose inline
//! content is dense in exactly one family of constructs, plus one plain-prose
//! baseline (same text volume, nothing to recognize) and one mixed document
//! that combines them under realistic block structure. A regression in one
//! pass moves its own benchmark and the mixed one; a regression in the shared
//! sweep overhead moves the baseline too.

use asciidoc_parser::Parser;
use codspeed_criterion_compat::{Criterion, black_box, criterion_group, criterion_main};

/// Repeats `paragraphs` in order until `count` paragraphs have been emitted,
/// separated by blank lines — deterministic, so every run parses identical
/// bytes.
fn repeat_paragraphs(paragraphs: &[&str], count: usize) -> String {
    let mut out = String::new();

    for i in 0..count {
        out.push_str(paragraphs[i % paragraphs.len()]);
        out.push_str("\n\n");
    }

    out
}

/// Prose with nothing for the inline passes to recognize: the baseline cost
/// of moving paragraph text through the substitution pipeline.
fn plain_prose() -> String {
    let text = repeat_paragraphs(
        &[
            "The renderer walks the document tree in source order and emits one \
             element per block, keeping the output stable across runs so that \
             golden files can pin every byte of it.",
            "Sections nest to any depth the source asks for, and each level \
             carries its own identifier derived from the title text according \
             to fixed and documented rules.",
            "A paragraph that spans several source lines is folded into a \
             single flow of text, and the line breaks the author typed are not \
             visible in the output unless explicitly requested.",
        ],
        60,
    );

    format!("= Plain prose\n\n{text}")
}

/// Prose dense in quoted-text formatting: every paragraph is swept by all of
/// the quote substitution rules and most paragraphs match several of them.
fn formatted_prose() -> String {
    let text = repeat_paragraphs(
        &[
            "The *renderer* walks the _document tree_ in `source order` and \
             emits **one** element per block, keeping the #output# stable \
             across runs.",
            "\"`Sections`\" nest to any depth the '`source`' asks for, and \
             each level carries its ^own^ identifier derived from the ~title~ \
             text.",
            "A paragraph with *_nested emphasis_* and a [.term]#styled span# \
             is folded into a `single` flow of **unconstrained** text.",
        ],
        60,
    );

    format!("= Formatted prose\n\n{text}")
}

/// Prose dense in inline macros and references: links, cross references,
/// footnotes, inline images, and index terms.
fn macro_prose() -> String {
    let text = repeat_paragraphs(
        &[
            "See https://example.com/reference[the reference] and \
             link:guide.html[the guide] for details, or start from \
             https://example.com directly.",
            "The behavior is defined in <<spec-section,the specification>> \
             and refined in xref:notes.adoc#errata[the errata] \
             list.footnote:[Errata are published quarterly.]",
            "Each entry carries an icon image:status.png[status,16,16] beside \
             the ((indexed term)) it documents, as shown in \
             <<fig-overview>>.",
        ],
        60,
    );

    // Both anchors the paragraphs reference are defined, so every repetition
    // exercises reference *resolution* rather than the unresolved-reference
    // fallback.
    format!(
        "= Macro prose\n\n[[spec-section]]\n== Specification\n\n\
         [[fig-overview]]\n.Overview\nThe anchored overview figure text.\n\n{text}"
    )
}

/// Prose dense in character replacements: typographic apostrophes, dashes,
/// ellipses, arrows, symbol replacements, and character references.
fn replacement_prose() -> String {
    let text = repeat_paragraphs(
        &[
            "It's the author's own text--every line of it--that decides what \
             the output holds... and the pipeline's job is fidelity.",
            "Copyright (C) and trademark (TM) marks render as symbols, (R) \
             included, and a -> arrow or a => arrow reads as one glyph.",
            "A reference like &copy; passes through as itself, a <- arrow \
             points back, and the em dash -- spaced this time -- stays \
             spaced.",
        ],
        60,
    );

    format!("= Replacement prose\n\n{text}")
}

/// A small but structurally realistic document: sections, lists, a table, a
/// source block, an admonition, and a quote block, all carrying formatted
/// inline content.
fn mixed_document() -> String {
    let section = "== Section {counter:sec}

The *renderer* emits '`stable`' output--see <<ref,the reference>> and
https://example.com/spec[the spec] for the `details`.footnote:[Every
release is checked against the golden corpus.]

.Highlights
* A _formatted_ list item with a link:notes.html[link]
* An item with `monospace` and a (C) mark
* An item with **strong** text and an image:dot.png[dot,8,8]

[source,ruby]
----
puts 'source blocks carry no inline substitution'
----

NOTE: An admonition's text is *substituted* like any paragraph's.

|===
|Column _one_ |Column *two*

|It's a cell--with replacements...
|A cell with a <<ref>> reference
|===

[quote,An Author]
A quoted block's attribution -> rendered with care.
";

    format!(
        "= Mixed document\n:toc:\n\n[[ref]]\n== Reference\n\nAnchor text.\n\n{}",
        section.repeat(12)
    )
}

pub fn throughput(c: &mut Criterion) {
    let cases = [
        ("throughput/plain_prose", plain_prose()),
        ("throughput/formatted_prose", formatted_prose()),
        ("throughput/macro_prose", macro_prose()),
        ("throughput/replacement_prose", replacement_prose()),
        ("throughput/mixed_document", mixed_document()),
    ];

    for (name, text) in &cases {
        c.bench_function(name, |b| {
            b.iter(|| Parser::default().parse(black_box(text.as_str())))
        });
    }
}

criterion_group!(benches, throughput);
criterion_main!(benches);
