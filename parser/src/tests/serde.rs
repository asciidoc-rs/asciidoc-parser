//! Verifies that a parsed [`Document`](crate::Document) and the AST / output
//! types it exposes serialize under the `serde` feature (see
//! <https://github.com/asciidoc-rs/asciidoc-parser/issues/895>).
//!
//! Only serialization is provided: the parsed model is a read-only projection
//! of the source, so a consumer that needs to reload a parse result re-parses
//! the original AsciiDoc rather than deserializing.

use crate::{Parser, blocks::FindBlocks};

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
