//! Verifies that a parsed [`Document`](crate::Document) and the AST / output
//! types it exposes serialize under the `serde` feature (see
//! <https://github.com/asciidoc-rs/asciidoc-parser/issues/895>).
//!
//! Only serialization is provided: the parsed model is a read-only projection
//! of the source, so a consumer that needs to reload a parse result re-parses
//! the original AsciiDoc rather than deserializing.

use crate::{
    Parser, SafeMode, blocks::FindBlocks, tests::prelude::inline_file_handler::InlineFileHandler,
};

const SAMPLE: &str = "= Doc Title\n\
:author: A. Uthor\n\
\n\
Intro paragraph with *bold* text.\n\
\n\
== A Section\n\
\n\
* item one\n\
* item two\n\
\n\
|===\n\
| A | B\n\
\n\
| 1 | 2\n\
|===\n";

#[test]
fn document_serializes_to_json() {
    let doc = Parser::default().parse(SAMPLE);

    let value: serde_json::Value =
        serde_json::to_value(&doc).expect("Document should serialize to JSON");

    // `Document` delegates to its parsed model, whose public members appear as
    // top-level keys.
    assert!(value.get("header").is_some(), "expected a `header` member");
    assert!(
        value.get("blocks").and_then(|b| b.as_array()).is_some(),
        "expected a `blocks` array"
    );
    assert!(
        value.get("warnings").and_then(|w| w.as_array()).is_some(),
        "expected a `warnings` array"
    );
}

#[test]
fn a_single_block_serializes() {
    let doc = Parser::default().parse("A paragraph.\n");

    let block = doc
        .child_blocks()
        .next()
        .expect("document should have one block");

    let json = serde_json::to_string(block).expect("Block should serialize");
    assert!(!json.is_empty());
}

#[test]
fn markdown_blockquote_owned_blocks_serialize() {
    // A Markdown-style (`>`) blockquote owns its `>`-stripped source, so its
    // nested blocks live in a `self_cell` (`OwnedQuoteBlocks`) reached only on
    // this path. Serializing the document exercises that cell's manual
    // `Serialize` impl.
    let doc = Parser::default().parse("> a quoted line\n> another quoted line\n");

    let json = serde_json::to_string(&doc).expect("Document with a blockquote should serialize");
    assert!(json.contains("quoted line"));
}

#[test]
fn owned_asciidoc_table_cell_serializes() {
    // An AsciiDoc (`a|`) table cell that expands an `include::` directive owns
    // its preprocessed source in a `self_cell` (`OwnedCell`) reached only on
    // this path. Serializing the document exercises that cell's manual
    // `Serialize` impl.
    let handler = InlineFileHandler::from_pairs([("inc.adoc", "included paragraph\n")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("|===\na| include::inc.adoc[]\n|===");

    let json = serde_json::to_string(&doc).expect("Document with an owned cell should serialize");
    assert!(json.contains("included paragraph"));
}
