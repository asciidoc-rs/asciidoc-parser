//! Regression coverage for locating preprocessor-directive warnings that arise
//! inside an AsciiDoc table cell.
//!
//! A malformed/unterminated conditional (or a tag-filter diagnostic) produces
//! no output of its own, so it carries a pre-resolved `origin`. When such a
//! warning arises in a file that a document-level cell's directive *included*,
//! that origin names the real `(file, line)` and must survive the cell's
//! warning reconstruction – it must not be discarded and re-anchored to the
//! cell's own line.

use crate::{
    Parser, SafeMode, parser::SourceLine, tests::prelude::inline_file_handler::InlineFileHandler,
    warnings::WarningType,
};

#[test]
fn conditional_warning_in_cell_included_content_keeps_its_origin() {
    // The document-level AsciiDoc cell's first line includes `inc.adoc`, whose
    // third line is a stray (unmatched) `endif`.
    let handler = InlineFileHandler::from_pairs([("inc.adoc", "line one\nline two\nendif::foo[]")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("|===\na| include::inc.adoc[]\n|===");

    let unmatched: Vec<_> = doc
        .warnings()
        .filter(|w| {
            matches!(
                &w.warning,
                WarningType::UnmatchedConditionalDirective(d) if d == "endif::foo[]"
            )
        })
        .collect();

    assert_eq!(unmatched.len(), 1, "expected one unmatched-endif warning");

    // The origin points at the *included* file and line, not the cell's own line.
    assert_eq!(
        unmatched[0].origin,
        Some(SourceLine(Some("inc.adoc".to_owned()), 3))
    );
}
