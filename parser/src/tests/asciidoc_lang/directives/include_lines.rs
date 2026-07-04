use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

track_file!("docs/modules/directives/pages/include-lines.adoc");

/// A target whose paragraphs sit on known line numbers, used to demonstrate
/// line-range selection:
///
/// ```text
/// 1: apple
/// 2:
/// 3: banana
/// 4:
/// 5: cherry
/// ```
fn nums_handler() -> InlineFileHandler {
    InlineFileHandler::from_pairs([("nums.adoc", "apple\n\nbanana\n\ncherry")])
}

fn included_paragraphs(attrs: &str) -> Vec<String> {
    let source = format!("include::nums.adoc[{attrs}]");
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(nums_handler())
        .parse(&source);
    rendered_paragraphs(&doc)
}

non_normative!(
    r#"
= Include Content by Line Ranges

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
The include directive supports selecting portions of the document to include.
Using the `lines` attribute, you can include ranges of line numbers.

When including multiple line ranges, each entry in the list must be separated by either a comma or a semicolon.
If commas are used, the entire value must be enclosed in quotes.
Using the semicolon as the data separator eliminates this requirement.

"#
    );

    // Selecting a subset of lines includes only that portion of the target.
    let paras = included_paragraphs("lines=1..3");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["apple", "banana"]);
}

non_normative!(
    r#"
== Specifying line ranges

"#
);

#[test]
fn single_range() {
    verifies!(
        r#"
To include content by line range, assign a starting line number and an ending line number separated by a pair of dots (e.g., `lines=1..5`) to the `lines` attribute.

"#
    );

    let paras = included_paragraphs("lines=3..5");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["banana", "cherry"]);
}

non_normative!(
    r#"
----
include::example$include.adoc[tag=line]
----

"#
);

#[test]
fn multiple_ranges_comma() {
    verifies!(
        r#"
You can specify multiple ranges by separating each range by a comma.
Since commas are normally used to separate individual attributes, you must quote the comma-separated list of ranges.

"#
    );

    // A comma-separated list of ranges must be quoted in the directive.
    let paras = included_paragraphs("lines=\"1..2,5..5\"");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["apple", "cherry"]);
}

non_normative!(
    r#"
----
include::example$include.adoc[tag=m-line-comma]
----

"#
);

#[test]
fn multiple_ranges_semicolon() {
    verifies!(
        r#"
To avoid having to quote the list of ranges, you can instead separate them using semicolons.

"#
    );

    // The semicolon separator does not require quotes.
    let paras = included_paragraphs("lines=1..2;5..5");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["apple", "cherry"]);
}

non_normative!(
    r#"
----
include::example$include.adoc[tag=m-line]
----

"#
);

#[test]
fn last_line_negative_one() {
    verifies!(
        r#"
If you don't know the number of lines in the document, or you don't want to couple the range to the length of the file, you can refer to the last line of the document using the value -1.

"#
    );

    let paras = included_paragraphs("lines=3..-1");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["banana", "cherry"]);
}

non_normative!(
    r#"
----
include::example$include.adoc[tag=last]
----

"#
);

#[test]
fn endless_range() {
    verifies!(
        r#"
Alternately, you can leave the end range unspecified and it will default to -1.

"#
    );

    let paras = included_paragraphs("lines=3..");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["banana", "cherry"]);
}

non_normative!(
    r#"
----
include::example$include.adoc[tag=endless]
----
"#
);
