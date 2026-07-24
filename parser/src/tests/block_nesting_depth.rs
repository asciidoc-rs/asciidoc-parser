//! Regression coverage for the block-nesting depth cap (issue #885).
//!
//! Block parsing descends recursively – a delimited block's body, a section
//! body, a table cell, and a nested list each parse on a fresh call stack – so
//! without a bound a small crafted document can overflow the native stack and
//! abort the whole process (an *uncatchable* failure). The `max-block-nesting`
//! attribute (default 32, API-only) caps that recursion: past the limit the
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
/// default-capped structure still holds one live stack frame per level, and a
/// debug build's frames are several times larger than a release build's. So in
/// debug this can need a couple MiB of stack even though it fits the normal
/// stack comfortably in release (where hosts actually run). Parsing on a large,
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
        limits.iter().all(|&l| l == 32),
        "every warning should report the default limit of 32, got {limits:?}"
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
        limits.iter().all(|&l| l == 32),
        "every warning should report the default limit of 32, got {limits:?}"
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
fn default_limit_is_32() {
    // The shipped default, mirroring `max-include-depth`.
    assert_eq!(
        Parser::default().attribute_value("max-block-nesting"),
        InterpretedValue::Value("32".to_string()),
    );
}

#[test]
fn limit_is_coerced_like_ruby_to_i() {
    // Resolves the configured attribute directly, exercising each coercion arm.
    fn cap_of(value: &str) -> usize {
        Parser::default()
            .with_intrinsic_attribute("max-block-nesting", value, ModificationContext::ApiOnly)
            .max_block_nesting()
    }

    // The default resolves to the built-in.
    assert_eq!(Parser::default().max_block_nesting(), 32);

    // A positive value is honored (trailing garbage coerced away as `to_i`).
    assert_eq!(cap_of("10"), 10);
    assert_eq!(cap_of("8bogus"), 8);

    // A non-positive value yields 0, permitting only the outermost scope.
    assert_eq!(cap_of("0"), 0);
    assert_eq!(cap_of("-5"), 0);

    // Both non-`Value` forms are reachable only through the API (the attribute
    // is API-only): an empty (`Set`) value coerces to 0, while an explicit unset
    // falls back to the default.
    assert_eq!(
        Parser::default()
            .with_intrinsic_attribute_bool("max-block-nesting", true, ModificationContext::ApiOnly)
            .max_block_nesting(),
        0
    );
    assert_eq!(
        Parser::default()
            .with_intrinsic_attribute_bool("max-block-nesting", false, ModificationContext::ApiOnly)
            .max_block_nesting(),
        32
    );
}

#[test]
fn limit_of_zero_refuses_all_nesting() {
    // A limit of 0 permits only the outermost, document-level scope: the block
    // itself still parses, but its (non-empty) nested content is truncated with
    // a warning reporting the limit.
    let doc = Parser::default()
        .with_intrinsic_attribute("max-block-nesting", "0", ModificationContext::ApiOnly)
        .parse("====\nnested paragraph\n====");
    let limits = nesting_warning_limits(&doc);

    assert!(
        !limits.is_empty(),
        "expected nesting to be refused at limit 0"
    );
    assert!(
        limits.iter().all(|&l| l == 0),
        "the warning should report the configured limit of 0, got {limits:?}"
    );
}

#[test]
fn empty_over_nested_scope_is_truncated_silently() {
    // An over-nested scope with no content is dropped without a warning –
    // nothing is lost, so there is nothing to report.
    let doc = Parser::default()
        .with_intrinsic_attribute("max-block-nesting", "0", ModificationContext::ApiOnly)
        .parse("====\n====");

    assert!(
        nesting_warning_limits(&doc).is_empty(),
        "an empty over-nested scope must not warn"
    );
}

#[test]
fn nested_list_after_separated_metadata_is_capped() {
    // A nested list reached via the "block metadata (anchor/attrlist) separated
    // by empty lines" path is bounded by the same guard as a directly-adjacent
    // nested list. With a limit of 0 the first nesting is refused.
    let doc = Parser::default()
        .with_intrinsic_attribute("max-block-nesting", "0", ModificationContext::ApiOnly)
        .parse("* parent\n[[anchor]]\n\n** child");

    assert!(
        !nesting_warning_limits(&doc).is_empty(),
        "expected the metadata-separated nested list to be capped"
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
        InterpretedValue::Value("32".to_string()),
        "the document assignment must not change the effective cap"
    );
}
