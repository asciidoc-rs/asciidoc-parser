use crate::{blocks::Block, tests::prelude::*};

track_file!("docs/modules/ROOT/pages/comments.adoc");

// NOTE ON DATA-MODEL DEVIATION: The AsciiDoc language specifies that comments
// are dropped from the parsed document. This parser deliberately deviates: it
// *retains* comments in the parsed model so that tooling can round-trip or
// inspect them. Comment blocks (in all three forms — `////`, a `[comment]` open
// block, and a `[comment]` paragraph) are retained as blocks with the `comment`
// context; comment lines are retained either in `Header::comments` (header) or
// as empty-rendered paragraph blocks (body). The two spec sentences that assert
// comments are "not included" / "dropped" are therefore mirrored here as
// `non_normative!` rather than `verifies!`.

#[test]
fn overview() {
    non_normative!(
        r#"
= Comments

Like programming languages, AsciiDoc provides a way to add commentary to your document that's not carried over into the published document.
This artifact is collectively known as a [.term]*comment*.
Putting text in a comment is often referred to as "`commenting it out`".

A comment is often used to insert a writer-facing notation or to hide draft content that's not ready for publishing.
In general, you can use comments anytime you want to hide lines from the processor.
Comments can also be useful as a processor hint to keep adjacent blocks of content separate.

"#
    );

    // DEVIATION: the language drops comments; this parser retains them (see the
    // note at the top of this file), so this sentence is non-normative here.
    non_normative!(
        r#"
The AsciiDoc processor will ignore comments during parsing and, thus, will not include them in the parsed document.
"#
    );

    verifies!(
        r#"
It will, however, account for the lines when mapping line numbers back to the source document.

"#
    );

    // A comment line does not disturb the line numbering of the blocks that
    // follow it: the trailing paragraph still reports its true source line (5),
    // accounting for the comment line (3) and the blank lines around it.
    let doc = Parser::default().parse("first\n\n// a comment\n\nsecond");
    let second = doc
        .nested_blocks()
        .find(|b| b.span().data() == "second")
        .expect("expected the 'second' paragraph");
    assert_eq!(second.span().line(), 5);

    non_normative!(
        r#"
AsciiDoc supports two styles of comments, line and block.
A line comment is for making comments line-by-line (i.e., comment line).
A block comment is for enclosing an arbitrary range of lines as a comment (i.e., comment block).

"#
    );
}

#[test]
fn comment_lines() {
    verifies!(
        r#"
[#comment-lines]
== Comment lines

A comment line is any line outside of a verbatim block that begins with a double forward slash (`//`) that's not immediately followed by a third forward slash.
Following this prefix, the line may contain an arbitrary number of characters.
It's customary to insert a single space between the prefix and the comment text (e.g., `// line comment`).

"#
    );

    // A `//` line is a comment (dropped from the rendered output)...
    let doc = Parser::default().parse("text\n// a comment\nmore");
    assert_eq!(rendered_paragraphs(&doc), vec!["text\nmore".to_string()]);

    // ...but a `///` line (a third forward slash) is not a comment; it stays as
    // ordinary content.
    let doc = Parser::default().parse("/// not a comment");
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["/// not a comment".to_string()]
    );

    non_normative!(
        r#"
.Line comment syntax
----
// A single-line comment.
----

"#
    );

    verifies!(
        r#"
When the processor encounters a line comment, it ignores the line and continues processing as though the line is not there.
Line comments are processed as lines are read, so they can be used where paragraph text is not permitted, such as between attribute entries in the document header.

"#
    );

    // A comment line between two attribute entries in the header is skipped and
    // retained on the header, without breaking the header.
    let doc = Parser::default().parse("= Title\n:a: 1\n// between attributes\n:b: 2\n\nbody");
    assert_eq!(doc.header().attributes().count(), 2);
    assert_eq!(doc.header().comments().count(), 1);

    verifies!(
        r#"
Line comments can be used strategically to separate blocks that otherwise have affinity, such as two adjacent lists.

"#
    );

    // A lone `//` between two lists forces them apart into two separate lists.
    let doc = Parser::default().parse("* first list\n\n//\n\n* second list");
    let lists = doc
        .nested_blocks()
        .filter(|b| matches!(b, Block::List(_)))
        .count();
    assert_eq!(lists, 2);

    non_normative!(
        r#"
.Line comment separating two lists
----
* first list

//

* second list
----

"#
    );

    // DEVIATION: the comment line is retained as an empty-rendered paragraph
    // rather than dropped, but it still serves as the block boundary described
    // here.
    non_normative!(
        r#"
In this case, the single line comment effectively acts as an empty paragraph that's dropped from the parsed document.
But before then, it will have served its purpose as a block boundary.

"#
    );
}

#[test]
fn comment_blocks() {
    verifies!(
        r#"
== Comment blocks

A comment block is a specialized delimited block.
It consists of an arbitrary number of lines bounded on either side by `////` delimiter lines.

"#
    );

    // A `////` delimited block parses as a block with the `comment` context.
    let doc = Parser::default().parse("////\nA comment block.\n////");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.raw_context().as_ref(), "comment");
    assert_eq!(block.resolved_context().as_ref(), "comment");

    verifies!(
        r#"
A comment block can be used anywhere a delimited block is normal accepted.
"#
    );

    // DEVIATION: the block is retained (with the `comment` context) instead of
    // being dropped from the parsed document.
    non_normative!(
        r#"
The main difference is that once the block is read, it's dropped from the parsed document (effectively ignored).
"#
    );

    verifies!(
        r#"
Additionally, no AsciiDoc syntax within the delimited lines is interpreted, not even preprocessor directives.

"#
    );

    // The content of a comment block is raw: neither inline markup nor an
    // attribute reference is interpreted, and an inner `//` line (a preprocessor
    // line comment) is retained verbatim rather than stripped.
    let doc = Parser::default().parse("////\n*bold* {attr}\n// inner\n////");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(
        block.rendered_content(),
        Some("*bold* {attr}\n// inner"),
        "comment block content must be raw"
    );

    // Even an explicit `subs` attribute does not cause a comment block to be
    // interpreted.
    let doc = Parser::default().parse("[subs=quotes]\n////\n*bold*\n////");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.rendered_content(), Some("*bold*"));

    non_normative!(
        r#"
.Block comment syntax
----
////
A comment block.

Notice it's a delimited block.
////
----

"#
    );

    verifies!(
        r#"
A comment block can also be written as an open block with the comment style:

"#
    );

    // An open block (`--`) carrying the `comment` style is the alternate form of
    // a comment block: same `comment` context, raw (uninterpreted) content —
    // inner `//` lines retained, and a `subs` override ignored — matching the
    // `////` form exactly.
    let doc = Parser::default().parse("[comment]\n--\n*bold* {attr}\n// inner\n--");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.resolved_context().as_ref(), "comment");
    assert_eq!(block.rendered_content(), Some("*bold* {attr}\n// inner"));

    let doc = Parser::default().parse("[comment,subs=quotes]\n--\n*bold*\n--");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.rendered_content(), Some("*bold*"));

    non_normative!(
        r#"
.Alternate block comment syntax
----
[comment]
--
A comment block.

Notice it's a delimited block.
--
----

"#
    );

    verifies!(
        r#"
A comment block that can consists of a single paragraph can be written as a paragraph with the comment style:

"#
    );

    // A `[comment]` paragraph is a comment: it carries the `comment` declared
    // style and its content is raw. A following, unstyled paragraph is ordinary
    // content.
    let doc = Parser::default()
        .parse("[comment]\nA paragraph comment.\nLike all paragraphs, the lines must be contiguous.\n\nNot a comment.");
    let first = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(first.declared_style(), Some("comment"));
    assert_eq!(first.substitution_group(), SubstitutionGroup::None);
    let second = doc.nested_blocks().nth(1).expect("expected a second block");
    assert_eq!(second.declared_style(), None);
    assert_eq!(second.rendered_content(), Some("Not a comment."));

    // Like the `////` and open-block forms, a `[comment]` paragraph retains its
    // content verbatim: an inner `//` line is preserved (not stripped as a line
    // comment), inline markup is not interpreted, and a `subs` override is
    // ignored.
    let doc = Parser::default().parse("[comment]\nfirst line\n// inner\n*bold*");
    let first = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(
        first.rendered_content(),
        Some("first line\n// inner\n*bold*")
    );

    let doc = Parser::default().parse("[comment,subs=quotes]\n*bold*");
    let first = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(first.rendered_content(), Some("*bold*"));

    non_normative!(
        r#"
.Comment paragraph syntax
----
[comment]
A paragraph comment.
Like all paragraphs, the lines must be contiguous.

Not a comment.
----

"#
    );

    verifies!(
        r#"
If comment blocks are used in a list item, they must be attached to the list item just like any other block.

"#
    );

    // A comment block attached with a list continuation (`+`) stays inside the
    // list item.
    let doc = Parser::default()
        .parse("* first item\n+\n////\nA comment block in a list.\n////\n\n* second item");
    assert_css(&doc, "ul", 1);
    assert_css(&doc, "ul > li", 2);

    non_normative!(
        r#"
.Block comment attached to list item
----
* first item
+
////
A comment block in a list.

Notice it's attached to the preceding list item.
////

* second item
----

"#
    );

    verifies!(
        r#"
Within a table, a comment block can only be used in an AsciiDoc table cell.

"#
    );

    // A comment block inside an AsciiDoc (`a|`) table cell parses without
    // disturbing the surrounding table.
    let doc = Parser::default()
        .parse("|===\na|\ncell text\n\n////\nA comment block in a table.\n////\n|===");
    assert_css(&doc, "table", 1);

    non_normative!(
        r#"
.Block comment within a table
----
|===
a|
cell text

////
A comment block in a table.

Notice the cell has the "a" (AsciiDoc) style.
////
|===
----

"#
    );

    non_normative!(
        r#"
Comment blocks can be very effective for commenting out sections of the document that are not ready for publishing or that provide background material or an outline for the text being drafted.
"#
    );
}
