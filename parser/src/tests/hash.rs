//! Verifies that the public AST / output types can be used as `HashMap` /
//! `HashSet` keys.

use std::collections::{HashMap, HashSet};

use crate::{
    Parser, Span,
    blocks::{Block, FindBlocks},
    warnings::WarningType,
};

#[test]
fn span_is_hashable() {
    let mut spans = HashSet::new();
    spans.insert(Span::new("abc"));
    spans.insert(Span::new("abc"));
    spans.insert(Span::new("def"));

    assert_eq!(spans.len(), 2);
}

#[test]
fn warning_type_buckets_by_variant() {
    let mut counts: HashMap<WarningType, usize> = HashMap::new();

    *counts.entry(WarningType::EmptyAttributeValue).or_default() += 1;
    *counts.entry(WarningType::EmptyAttributeValue).or_default() += 1;
    *counts.entry(WarningType::EmptyShorthandName).or_default() += 1;

    assert_eq!(counts[&WarningType::EmptyAttributeValue], 2);
    assert_eq!(counts[&WarningType::EmptyShorthandName], 1);
}

#[test]
fn blocks_and_content_key_a_hashset() {
    let doc = Parser::default().parse("Para one.\n\nPara two.\n");

    // `Block` (and, transitively, the `Content` it holds) can key a `HashSet`.
    let mut blocks: HashSet<&Block<'_>> = HashSet::new();
    for block in doc.child_blocks() {
        blocks.insert(block);
    }

    assert_eq!(blocks.len(), 2);
}

#[test]
fn a_table_with_an_asciidoc_cell_is_hashable() {
    // An AsciiDoc (`a|`) table cell carries a `ResolvedAttributes` snapshot,
    // whose `Hash` is hand-written (its `HashMap` fields can not derive it).
    // Hashing the enclosing block exercises that path.
    let doc = Parser::default().parse("|===\na| A nested _paragraph_.\n|===\n");

    let mut blocks: HashSet<&Block<'_>> = HashSet::new();
    for block in doc.child_blocks() {
        blocks.insert(block);
    }

    assert_eq!(blocks.len(), 1);
}
