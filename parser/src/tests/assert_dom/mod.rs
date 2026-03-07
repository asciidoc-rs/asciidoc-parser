//! DOM assertion utilities for testing.
//!
//! This module provides assertion functions that can be used in tests
//! to verify the structure of parsed AsciiDoc documents.

use crate::{Document, blocks::IsBlock};

/// Asserts that an XPath query matches exactly the expected number of nodes.
///
/// # Panics
///
/// Panics if the number of matches doesn't equal `expected_count`.
///
/// # Examples
///
/// ```ignore
/// let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
/// assert_xpath(&doc, "//ul", 1);
/// assert_xpath(&doc, "//li", 3);
/// ```
#[track_caller]
pub(crate) fn assert_xpath(doc: &Document, xpath: &str, expected_count: usize) {
    let vdom = doc.to_virtual_dom();
    let matches = query_xpath(&vdom, xpath);

    assert_eq!(
        matches.len(),
        expected_count,
        "XPath query '{}' expected {} matches, found {}",
        xpath,
        expected_count,
        matches.len()
    );
}

/// Asserts that a CSS selector matches exactly the expected number of nodes.
///
/// # Panics
///
/// Panics if the number of matches doesn't equal `expected_count`.
///
/// # Examples
///
/// ```ignore
/// let doc = Parser::default().parse("* item");
/// assert_css(&doc, ".ulist", 1);
/// assert_css(&doc, "ul li", 1);
/// ```
#[track_caller]
pub(crate) fn assert_css(doc: &Document, selector: &str, expected_count: usize) {
    let vdom = doc.to_virtual_dom();
    let matches = query_css(&vdom, selector);

    assert_eq!(
        matches.len(),
        expected_count,
        "CSS query '{}' expected {} matches, found {}",
        selector,
        expected_count,
        matches.len()
    );
}

/// Asserts that at least one node contains the designated text in its rendered
/// content.
///
/// # Panics
///
/// Panics if no node's `content` (rendered content) contains the provided
/// text.
///
/// # Examples
///
/// ```ignore
/// let doc = Parser::default().parse("* item");
/// assert_output_contains(&doc, "item"); // will not panic
/// assert_output_contains(&doc, "nonsense"); // will panic
/// ```
pub(crate) fn assert_output_contains<'src>(doc: &'src Document<'src>, text: &str) {
    if !check_block_contains(doc, text) {
        panic!("Document should have contained {text}, but did not:\n\n{doc:#?}");
    }
}

fn check_block_contains<'src, B: IsBlock<'src>>(block: &'src B, text: &str) -> bool {
    if let Some(content) = block.rendered_content()
        && content.contains(text)
    {
        return true;
    }

    block.nested_blocks().any(|b| check_block_contains(b, text))
}

/// Refutes that any node contains the designated text in its rendered content.
///
/// # Panics
///
/// Panics if any node's `content` (rendered content) contains the provided
/// text.
///
/// # Examples
///
/// ```ignore
/// let doc = Parser::default().parse("* item");
/// refute_output_contains(&doc, "item"); // will panic
/// refute_output_contains(&doc, "nonsense"); // will not panic
/// ```
pub(crate) fn refute_output_contains<'src>(doc: &'src Document<'src>, text: &str) {
    refute_block_contains(doc, text);
}

fn refute_block_contains<'src, B: IsBlock<'src>>(block: &'src B, text: &str) {
    if let Some(content) = block.rendered_content() {
        dbg!(&content);
        if content.contains(text) {
            panic!("Block should not have contained {text}, but did:\n\n{block:#?}");
        }
    }

    for nested_block in block.nested_blocks() {
        refute_block_contains(nested_block, text);
    }
}

mod css;
use css::*;

mod virtual_dom;
pub(crate) use virtual_dom::ToVirtualDom;

mod xpath;
pub(crate) use xpath::query_xpath;

#[cfg(test)]
mod tests {
    mod xpath {
        use super::super::*;
        use crate::Parser;

        #[test]
        fn assert_xpath_success() {
            let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//li", 3);
        }

        #[test]
        #[should_panic(expected = "XPath query '//ul' expected 2 matches, found 1")]
        fn assert_xpath_failure() {
            let doc = Parser::default().parse("* item");
            assert_xpath(&doc, "//ul", 2); // Should panic
        }

        #[test]
        fn assert_xpath_with_path() {
            let doc = Parser::default().parse("== Section\n\n* item 1\n* item 2");
            assert_xpath(&doc, "//div/ul/li", 2);
        }

        #[test]
        fn assert_xpath_with_predicate() {
            let doc = Parser::default().parse("Hello\n\nWorld");
            assert_xpath(&doc, "//p[text()=\"Hello\"]", 1);
        }

        #[test]
        fn assert_xpath_with_single_quoted_text_and_newline() {
            // This matches the failing test from lists_test.rs.
            let doc =
                Parser::default().parse("List\n====\n\n- Foo\nwrapped content\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li[1]/p[text() = \'Foo\\nwrapped content\']", 1);
        }
    }

    mod css {
        use super::super::*;
        use crate::Parser;

        #[test]
        fn assert_css_success() {
            let doc = Parser::default().parse("* item");
            assert_css(&doc, "ul", 1);
        }

        #[test]
        fn assert_css_with_class_selector() {
            let doc = Parser::default().parse("* item");
            assert_css(&doc, ".ulist", 1);
        }
    }

    mod refute_output_contains {
        use super::super::*;
        use crate::Parser;

        #[test]
        fn succeeds_when_no_such_content() {
            let doc = Parser::default().parse("* item 1\n* item 2\n* item 3");
            refute_output_contains(&doc, "blah");
        }

        #[test]
        #[should_panic]
        fn panics_when_content_found_directly() {
            let doc = Parser::default().parse("On the wolpertanger hunt");
            refute_output_contains(&doc, "wolpertanger");
        }

        #[test]
        #[should_panic]
        fn panics_when_content_found_nested() {
            let doc = Parser::default().parse("* item 1\n** item 2\n* item 3");
            refute_output_contains(&doc, "item 2");
        }
    }
}
