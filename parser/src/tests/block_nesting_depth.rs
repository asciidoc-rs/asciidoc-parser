//! Regression coverage for the block-nesting depth cap (issue #885).
//!
//! Block parsing descends recursively – a delimited block's body, a section
//! body, a table cell, and a nested list each parse on a fresh call stack – so
//! without a bound a small crafted document can overflow the native stack and
//! abort the whole process (an *uncatchable* failure). The `max-block-nesting`
//! attribute (default 64, API-only) caps that recursion: past the limit the
//! over-nested content is truncated with a
//! [`MaxBlockNestingExceeded`](WarningType::MaxBlockNestingExceeded) warning
//! instead of being descended into.

use crate::{
    Parser, document::InterpretedValue, parser::ModificationContext, warnings::WarningType,
};

/// Collects the limit reported by every
/// [`MaxBlockNestingExceeded`](WarningType::MaxBlockNestingExceeded) warning in
/// a document (there is one per truncation point; they should all agree).
fn nesting_warning_limits(doc: &crate::Document<'_>) -> Vec<usize> {
    doc.warnings()
        .filter_map(|w| match &w.warning {
            WarningType::MaxBlockNestingExceeded(limit) => Some(*limit),
            _ => None,
        })
        .collect()
}

/// Parses `source` on a generously-sized thread and returns the limits reported
/// by its nesting-depth warnings.
///
/// The cap makes recursion *bounded*, which is what prevents the abort; but a
/// default-capped (64-deep) structure still holds 64 live stack frames, and a
/// debug build's frames are several times larger than a release build's. So in
/// debug this can need a few MiB of stack even though it fits the normal stack
/// comfortably in release (where hosts actually run). Parsing on a large,
/// explicitly-sized stack lets these tests exercise the cap without being
/// sensitive to the test harness's own thread-stack size.
fn nesting_warning_limits_on_large_stack(source: String) -> Vec<usize> {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || nesting_warning_limits(&Parser::default().parse(&source)))
        .expect("spawn parse thread")
        .join()
        .expect("parsing a pathologically-nested document must not overflow the stack")
}

#[test]
fn strictly_increasing_delimiters_are_capped_at_the_default() {
    // Each line is a longer example-block delimiter than the last, so it can
    // never close the block it sits inside – every line opens a *nested*
    // example block. Before the cap this drove unbounded recursion and aborted
    // with a stack overflow at a few hundred levels.
    let mut source = String::new();
    for n in 4..404 {
        source.push_str(&"=".repeat(n));
        source.push('\n');
    }

    let limits = nesting_warning_limits_on_large_stack(source);

    assert!(
        !limits.is_empty(),
        "expected at least one nesting-depth warning"
    );
    assert!(
        limits.iter().all(|&l| l == 64),
        "every warning should report the default limit of 64, got {limits:?}"
    );
}

#[test]
fn deeply_nested_list_markers_are_capped_at_the_default() {
    // Strictly-increasing unordered-list markers (`*`, `**`, `***`, …) nest
    // without bound; before the cap ~1,200 levels aborted with a stack
    // overflow.
    let mut source = String::new();
    for depth in 1..=400 {
        source.push_str(&"*".repeat(depth));
        source.push_str(" item\n");
    }

    let limits = nesting_warning_limits_on_large_stack(source);

    assert!(
        !limits.is_empty(),
        "expected at least one nesting-depth warning"
    );
    assert!(
        limits.iter().all(|&l| l == 64),
        "every warning should report the default limit of 64, got {limits:?}"
    );
}

#[test]
fn shallow_nesting_is_not_capped() {
    // A modestly-nested document (well under the default limit) parses cleanly
    // with no nesting-depth warning.
    let source = "\
====
outer

=====
middle

======
inner
======
=====
====
";

    let doc = Parser::default().parse(source);

    assert!(
        nesting_warning_limits(&doc).is_empty(),
        "a shallow document must not be capped"
    );
}

#[test]
fn lowered_limit_is_honored() {
    // A host on a small stack can lower the cap. With a limit of 2, a
    // five-deep delimiter nest is truncated and the warning reports the
    // configured limit. (A limit this low keeps the structure shallow, so the
    // test does not depend on the ambient stack size.)
    let mut source = String::new();
    for n in 4..9 {
        source.push_str(&"=".repeat(n));
        source.push('\n');
    }

    let doc = Parser::default()
        .with_intrinsic_attribute("max-block-nesting", "2", ModificationContext::ApiOnly)
        .parse(&source);
    let limits = nesting_warning_limits(&doc);

    assert!(!limits.is_empty(), "expected the lowered cap to fire");
    assert!(
        limits.iter().all(|&l| l == 2),
        "the warning should report the configured limit of 2, got {limits:?}"
    );
}

#[test]
fn default_limit_is_64() {
    // The shipped default, mirroring `max-include-depth`.
    assert_eq!(
        Parser::default().attribute_value("max-block-nesting"),
        InterpretedValue::Value("64".to_string()),
    );
}

#[test]
fn limit_cannot_be_raised_by_the_document() {
    // `max-block-nesting` is API-only: a document-body assignment is rejected
    // (with the usual locked-attribute warning) and the effective cap keeps its
    // default, so a hostile document cannot raise its own limit.
    let mut parser = Parser::default();
    let doc = parser.parse(":max-block-nesting: 100000\n\nhello");

    assert!(
        doc.warnings().any(|w| matches!(
            &w.warning,
            WarningType::AttributeValueIsLocked(name) if name == "max-block-nesting"
        )),
        "expected a locked-attribute warning for the rejected assignment"
    );

    assert_eq!(
        parser.attribute_value("max-block-nesting"),
        InterpretedValue::Value("64".to_string()),
        "the document assignment must not change the effective cap"
    );
}
