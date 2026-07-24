//! End-to-end tests for the [`Origin`] / [`Fidelity`] API: mapping a position
//! in the preprocessed source back to the original input file, line, and (on
//! verbatim lines) column, across the preprocessor transforms that change line
//! content.
//!
//! [`Origin`]: crate::parser::Origin
//! [`Fidelity`]: crate::parser::Fidelity

use crate::{
    SafeMode,
    parser::{Fidelity, ModificationContext, Origin, Transform},
    tests::prelude::{inline_file_handler::InlineFileHandler, *},
};

/// Root content that never went through a content-changing transform maps
/// straight back: `None` file, matching line, and a column that carries
/// through.
#[test]
fn root_content_is_verbatim_with_column() {
    let doc = Parser::default().parse("= Title\n\nA paragraph.\n");

    // Line 3, column 3 of the preprocessed source is column 3 of line 3 of the
    // root document (there was no preprocessing, so the map is empty and every
    // position resolves to the root, verbatim).
    assert_eq!(
        doc.source_map().origin_at(3, 3),
        Origin {
            file: None,
            line: 3,
            col: Some(3),
            fidelity: Fidelity::Verbatim,
        }
    );
}

/// A line whose tabs were expanded by `tabsize` maps back to its origin file
/// and line, but reports no column (the expansion shifted every column after
/// the tab); a neighboring line the expansion did not touch stays verbatim.
#[test]
fn tab_expanded_include_line_is_transformed() {
    let handler = InlineFileHandler::from_pairs([("code.rb", "\ttabbed\nplain\n")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute("tabsize", "4", ModificationContext::Anywhere)
        .with_include_file_handler(handler)
        .parse("before\n\n----\ninclude::code.rb[]\n----\n");

    // The preprocessed source is:
    //   1: before
    //   2:
    //   3: ----
    //   4:     tabbed   (code.rb line 1, tab expanded)
    //   5: plain        (code.rb line 2, unchanged)
    //   6: ----
    assert_eq!(
        doc.source_map().origin_at(4, 1),
        Origin {
            file: Some("code.rb"),
            line: 1,
            col: None,
            fidelity: Fidelity::Transformed(Transform::TabExpansion),
        }
    );

    // The second included line had no tab, so its column maps through.
    assert_eq!(
        doc.source_map().origin_at(5, 1),
        Origin {
            file: Some("code.rb"),
            line: 2,
            col: Some(1),
            fidelity: Fidelity::Verbatim,
        }
    );

    // Root content after the include re-anchors to the root file, verbatim.
    assert_eq!(
        doc.source_map().origin_at(6, 1),
        Origin {
            file: None,
            line: 5,
            col: Some(1),
            fidelity: Fidelity::Verbatim,
        }
    );
}

/// A plainly included file (no indent/tabsize) is verbatim: its columns map
/// straight through to the origin file.
#[test]
fn plain_include_is_verbatim() {
    let handler = InlineFileHandler::from_pairs([("chapter.adoc", "Chapter body.\n")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("Intro.\n\ninclude::chapter.adoc[]\n");

    // Preprocessed line 3 is `Chapter body.`, from chapter.adoc line 1.
    assert_eq!(
        doc.source_map().origin_at(3, 9),
        Origin {
            file: Some("chapter.adoc"),
            line: 1,
            col: Some(9),
            fidelity: Fidelity::Verbatim,
        }
    );
}

/// An include that cannot be resolved becomes a synthetic "Unresolved
/// directive" line: it has no verbatim origin, so its column is unavailable and
/// its fidelity names the transform.
#[test]
fn unresolved_include_is_synthetic() {
    let handler = InlineFileHandler::from_pairs([]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("include::missing.adoc[]\n");

    let origin = doc.source_map().origin_at(1, 1);
    assert_eq!(origin.file, None);
    assert_eq!(origin.line, 1);
    assert_eq!(origin.col, None);
    assert_eq!(
        origin.fidelity,
        Fidelity::Synthetic(Transform::UnresolvedDirective)
    );
}

/// `Document::origin_of` resolves a parsed element's span through the source
/// map: the listing block below begins on a verbatim root line.
#[test]
fn origin_of_block_span() {
    let handler = InlineFileHandler::from_pairs([("code.rb", "puts 1\n")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("intro\n\n----\ninclude::code.rb[]\n----\n");

    let listing = doc.nested_blocks().nth(1).unwrap();
    let origin = doc.origin_of(listing.span());

    // The listing's `----` opener is root line 3, verbatim.
    assert_eq!(origin.file, None);
    assert_eq!(origin.line, 3);
    assert_eq!(origin.fidelity, Fidelity::Verbatim);
    assert_eq!(origin.col, Some(1));
}
