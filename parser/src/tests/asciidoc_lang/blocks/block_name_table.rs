use crate::{blocks::ContentModel, tests::prelude::*};

track_file!("ref/asciidoc-lang/docs/modules/blocks/partials/block-name-table.adoc");

non_normative!(
    r#"
////
Table of blocks, block names, block delimiters, and their substitutions
User Manual: Blocks
////

[cols="1,1m,1m,3,1"]
|===
|Block |Block Name |Delimiter |Purpose |Substitutions

"#
);

/// The block-identity facts for the first top-level block of `src`: its
/// resolved context, content model, and declared block style.
///
/// Returned as owned values so the parsed (borrowing) document can be dropped
/// at the end of this helper.
fn facts(src: &str) -> (String, ContentModel, Option<String>) {
    let doc = Parser::default().parse(src);
    let block = doc
        .nested_blocks()
        .next()
        .expect("expected at least one block");

    (
        block.resolved_context().to_string(),
        block.content_model(),
        block.declared_style().map(str::to_string),
    )
}

#[test]
fn paragraph() {
    verifies!(
        r#"
|Paragraph
d|n/a
d|n/a
|Regular paragraph content (i.e., prose), offset on either side by an empty line.
Must start flush to the left margin of the document.
The block name can be used to convert the paragraph into most other blocks.
|Normal

"#
    );

    let (resolved, model, _) = facts("Regular paragraph content.");
    assert_eq!(resolved, "paragraph");
    assert_eq!(model, ContentModel::Simple);
}

#[test]
fn literal_paragraph() {
    verifies!(
        r#"
|Literal paragraph
d|n/a
d|n/a
|A special type of paragraph block for literal content (i.e., preformatted text).
Must be indented from the left margin of the document by at least one space.
Often used as a shorthand for a literal delimited block when the content does not contain any empty lines.
|Verbatim

"#
    );

    // An indented paragraph is a literal paragraph: it keeps the simple content
    // model but is styled as a literal block and uses verbatim substitutions.
    let doc = Parser::default().parse("  literal output text");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.resolved_context().as_ref(), "paragraph");
    assert_eq!(block.content_model(), ContentModel::Simple);
    assert_eq!(block.substitution_group(), SubstitutionGroup::Verbatim);

    let crate::blocks::Block::Simple(simple) = block else {
        panic!("expected a simple block, got {block:?}");
    };
    assert_eq!(simple.style(), SimpleBlockStyle::Literal);
}

#[test]
fn admonition() {
    verifies!(
        r#"
|Admonition
|++[<LABEL>]++
|++====++
|Aside content that demands special attention; often labeled with a tag or icon
|Normal

"#
    );

    let (resolved, model, style) = facts("[NOTE]\n====\nPay attention.\n====");
    assert_eq!(resolved, "admonition");
    assert_eq!(model, ContentModel::Compound);
    assert_eq!(style.as_deref(), Some("NOTE"));
}

#[test]
fn comment() {
    verifies!(
        r#"
|Comment
d|n/a
|++////++
|Private notes that are not displayed in the output
|None

"#
    );

    let (resolved, model, _) = facts("////\nprivate note\n////");
    assert_eq!(resolved, "comment");
    assert_eq!(model, ContentModel::Raw);
}

#[test]
fn example() {
    verifies!(
        r#"
|Example
|++[example]++
|++====++
|Designates example content or defines an admonition block
|Normal

"#
    );

    let (resolved, model, _) = facts("====\nexample content\n====");
    assert_eq!(resolved, "example");
    assert_eq!(model, ContentModel::Compound);
}

#[test]
fn fenced() {
    verifies!(
        r#"
|Fenced
d|n/a
|++```++
|Source code or keyboard input is displayed as entered.
Will be colorized by the source highlighter if enabled on the document and a language is set.
|Verbatim

"#
    );

    // A fenced code block (three backticks) is a shorthand for a listing block:
    // it resolves to the `listing` context with the verbatim content model. The
    // block has no declared style unless a `[source,<lang>]` attrlist is placed
    // above the opening fence.
    let (resolved, model, style) = facts("```\nsource code\n```");
    assert_eq!(resolved, "listing");
    assert_eq!(model, ContentModel::Verbatim);
    assert_eq!(style, None);
}

#[test]
fn listing() {
    verifies!(
        r#"
|Listing
|++[listing]++
|++----++
|Source code or keyboard input is displayed as entered
|Verbatim

"#
    );

    let (resolved, model, _) = facts("----\nsource code\n----");
    assert_eq!(resolved, "listing");
    assert_eq!(model, ContentModel::Verbatim);
}

#[test]
fn literal() {
    verifies!(
        r#"
|Literal
|++[literal]++
|++....++
|Output text is displayed exactly as entered
|Verbatim

"#
    );

    let (resolved, model, _) = facts("....\nliteral output\n....");
    assert_eq!(resolved, "literal");
    assert_eq!(model, ContentModel::Verbatim);
}

#[test]
fn open() {
    verifies!(
        r#"
|Open
d|Most block names
|++--++
|Anonymous block that can act as any block except _passthrough_ or _table_ blocks
|Varies

"#
    );

    let (resolved, model, _) = facts("--\nanonymous content\n--");
    assert_eq!(resolved, "open");
    assert_eq!(model, ContentModel::Compound);
}

#[test]
fn passthrough() {
    verifies!(
        r#"
|Passthrough
|++[pass]++
|pass:[++++]
|Unprocessed content that is sent directly to the output
|None

"#
    );

    let (resolved, model, _) = facts("++++\n<b>raw</b>\n++++");
    assert_eq!(resolved, "pass");
    assert_eq!(model, ContentModel::Raw);
}

#[test]
fn quote() {
    verifies!(
        r#"
|Quote
|++[quote]++
|++____++
|A quotation with optional attribution
|Normal

"#
    );

    let (resolved, model, _) = facts("____\na quotation\n____");
    assert_eq!(resolved, "quote");
    assert_eq!(model, ContentModel::Compound);
}

#[test]
fn sidebar() {
    verifies!(
        r#"
|Sidebar
|++[sidebar]++
|++****++
|Aside text and content displayed outside the flow of the document
|Normal

"#
    );

    let (resolved, model, _) = facts("****\naside\n****");
    assert_eq!(resolved, "sidebar");
    assert_eq!(model, ContentModel::Compound);
}

#[test]
fn source() {
    verifies!(
        r#"
|Source
|++[source]++
|++----++
|Source code or keyboard input to be displayed as entered.
Will be colorized by the source highlighter if enabled on the document and a language is set.
|Verbatim

"#
    );

    // A `[source]` style over a listing (`----`) delimited block resolves to the
    // `listing` context, specialized by the `source` style.
    let (resolved, model, style) = facts("[source]\n----\ncode\n----");
    assert_eq!(resolved, "listing");
    assert_eq!(model, ContentModel::Verbatim);
    assert_eq!(style.as_deref(), Some("source"));
}

#[test]
fn stem() {
    verifies!(
        r#"
|Stem
|++[stem]++
|pass:[++++]
|Unprocessed content that is sent directly to an interpreter (such as AsciiMath or LaTeX math)
|None

"#
    );

    // A `[stem]` style over a passthrough (`++++`) delimited block resolves to
    // the `stem` context with the raw content model.
    let (resolved, model, style) = facts("[stem]\n++++\nx^2\n++++");
    assert_eq!(resolved, "stem");
    assert_eq!(model, ContentModel::Raw);
    assert_eq!(style.as_deref(), Some("stem"));
}

#[test]
fn table() {
    verifies!(
        r#"
|Table
d|n/a
|++\|===++
|Displays tabular content
|Varies

"#
    );

    let (resolved, model, _) = facts("|===\n|a |b\n|===");
    assert_eq!(resolved, "table");
    assert_eq!(model, ContentModel::Table);
}

#[test]
fn verse() {
    verifies!(
        r#"
|Verse
|++[verse]++
|++____++
|A verse with optional attribution
|Normal
|===
"#
    );

    // A `[verse]` style over a quote (`____`) delimited block resolves to the
    // `verse` context with the simple content model.
    let (resolved, model, style) = facts("[verse]\n____\na verse\n____");
    assert_eq!(resolved, "verse");
    assert_eq!(model, ContentModel::Simple);
    assert_eq!(style.as_deref(), Some("verse"));
}
