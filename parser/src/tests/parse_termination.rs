//! Regression coverage for parse termination (issue #1234).
//!
//! The block-collection loops advance by consuming the source a parsed block
//! stands for. They are bounded on the source being *non-empty*, not on it
//! having *moved*, so an outcome that reports no progress spins forever rather
//! than failing visibly — an infinite loop inside a library call, which a host
//! parsing untrusted AsciiDoc cannot defend against with a timeout.
//!
//! The inputs below all reach that state through the same door: a line whose
//! only content is a vertical tab (U+000B), a form feed (U+000C), or carriage
//! returns. [`Span::take_empty_line`](crate::Span::take_empty_line) counts a
//! line as blank only when every byte is a space or a tab, so such a line is
//! never skipped as blank; the block parsers, which trim trailing whitespace by
//! the Unicode definition, then find nothing in it to build a block from. The
//! line is not blank enough to skip and not substantial enough to parse.
//!
//! Whether such a line *should* count as blank is a language question, and
//! these tests deliberately do not settle it: they assert only that parsing
//! terminates, and that content following the offending line still parses.

use std::{sync::mpsc, thread, time::Duration};

use crate::{HasSpan, Parser, blocks::Block};

/// Parses `source` on its own thread and reports whether the call returned.
///
/// The parse cannot run on the test thread: before the fix it never returned,
/// so a direct call would not fail the test — the run would die on a harness
/// timeout, with no named assertion pointing at the input that hung. Off-thread
/// the failure is a normal assertion; the spinning thread is leaked, which the
/// test process cleans up when it exits.
fn terminates(source: &'static str) -> bool {
    let (done, finished) = mpsc::channel();

    thread::spawn(move || {
        let _ = Parser::default().parse(source);
        let _ = done.send(());
    });

    finished.recv_timeout(Duration::from_secs(10)).is_ok()
}

/// The five inputs from the original report, minimised by libFuzzer. The first
/// is a single byte.
#[test]
fn a_line_holding_only_a_control_character_terminates() {
    for source in [
        "\u{c}",
        ";toc::  \u{c}",
        "\n\r\r\r",
        "= T\n\n\r\r",
        "[;;\n\u{b}",
    ] {
        assert!(terminates(source), "parse did not terminate on {source:?}");
    }
}

/// The same line reached through each block scope that collects child blocks:
/// the document body, a section body, a compound delimited block, a quote
/// block, a table cell, and a list item's continuation. Each nests a separate
/// call to a block-collection loop, so each is a separate way in.
#[test]
fn such_a_line_terminates_in_every_block_scope() {
    for source in [
        "before\n\n\u{c}\n\nafter",
        "= T\n\n== S\n\n\u{c}\n\ntext",
        "====\nx\n\n\u{c}\n\ny\n====",
        "____\na\n\n\u{c}\n\nb\n____",
        "|===\n| a\n\n\u{c}\n\n| b\n|===",
        "* a\n+\n\u{c}\n",
        "\u{c}\n\u{b}\n\r",
    ] {
        assert!(terminates(source), "parse did not terminate on {source:?}");
    }
}

/// Terminating by abandoning the rest of the document would satisfy the tests
/// above while quietly discarding content, so pin down that the offending line
/// is skipped rather than treated as the end of the input.
#[test]
fn content_after_such_a_line_still_parses() {
    let doc = Parser::default().parse("before\n\n\u{c}\n\nafter");

    let paragraphs: Vec<&str> = doc
        .top_level_blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple.span().data()),
            _ => None,
        })
        .collect();

    assert_eq!(paragraphs, vec!["before", "after"]);
}
