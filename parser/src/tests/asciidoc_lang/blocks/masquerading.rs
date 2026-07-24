use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/masquerading.adoc");

non_normative!(
    r#"
= Block Masquerading
:page-aliases: masquerade.adoc

"#
);

/// The masquerade-relevant facts about the first top-level block of `src`: its
/// raw context, resolved context, content model, and declared style.
///
/// Returned as owned values so the parsed (borrowing) [`Document`] can be
/// dropped at the end of this helper.
///
/// [`Document`]: crate::Document
fn facts(src: &str) -> (String, String, ContentModel, Option<String>) {
    let doc = Parser::default().parse(src);
    let block = doc
        .child_blocks()
        .next()
        .expect("expected at least one block");

    (
        block.raw_context().to_string(),
        block.resolved_context().to_string(),
        block.content_model(),
        block.declared_style().map(str::to_string),
    )
}

#[test]
fn overview() {
    verifies!(
        r#"
The declared block style (i.e., the first positional attribute of the block attribute list) can be used to modify the context of any paragraph and most structural containers.
This practice is known as block masquerading (meaning to disguise one block as another).

When the context of a paragraph is changed using the declared block style, the block retains the simple content model.
When masquerading the context of a structural container, only contexts that preserve the expected content model are permitted.

"#
    );

    // The declared block style modifies the context of a paragraph: `[sidebar]`
    // on a paragraph changes its context to `sidebar`, and the block retains the
    // simple content model.
    let (_, resolved, model, _) = facts("[sidebar]\nA short aside.");
    assert_eq!(resolved, "sidebar");
    assert_eq!(model, ContentModel::Simple);

    // Masquerading the context of a structural container is permitted only for
    // contexts that preserve the expected content model. A `[listing]` style on
    // a `....` literal container changes the context to `listing`; both literal
    // and listing share the verbatim content model, so the masquerade is
    // permitted and the content model is preserved.
    let (_, resolved, model, _) = facts("[listing]\n....\na > b\n....");
    assert_eq!(resolved, "listing");
    assert_eq!(model, ContentModel::Verbatim);
}

#[test]
fn how_it_works() {
    non_normative!(
        r#"
== How it works

"#
    );

    verifies!(
        r#"
If the block style declared on a block matches the name of a context, it sets the context of the block to that value and the resolved block style will be left unset.
If the declared block style does not match the name of a context, it will either specialize the context or set the context implicitly and also specialize that context.
"#
    );

    // A declared block style that matches the name of a context sets the
    // context of the block to that value (here, `sidebar`).
    let (_, resolved, _, _) = facts("[sidebar]\nA short aside.");
    assert_eq!(resolved, "sidebar");

    // A declared block style that does *not* match the name of a context can
    // specialize the context: `source` is not a context, but it specializes the
    // `listing` context, so a `[source]` literal block resolves to `listing`.
    let (_, resolved, _, style) = facts("[source]\n....\na > b\n....");
    assert_eq!(style.as_deref(), Some("source"));
    assert_eq!(resolved, "listing");

    // A declared block style can also set the context implicitly and specialize
    // it: `NOTE` sets the context to `admonition` and also specializes the
    // admonition with the `NOTE` style.
    let (raw, _, _, _) = facts("[NOTE]\nRemember the milk.");
    assert_eq!(raw, "admonition");

    non_normative!(
        r#"
How the declared block style is handled for a custom block is up to the extension, though a similar process takes place.

"#
    );
}

#[test]
fn change_a_structural_container_context() {
    verifies!(
        r#"
Let's consider the case of using the declared block style to change the context of a structural container.
In this case, we're using the declared block style to change a literal block to a listing block.

----
[listing]
....
a > b
....
----

Even though the default context for the structural container is `:literal`, the declared block style changes it to `:listing`.
The resolved block style of the block remains unset.

"#
    );

    // The `....` delimiters mark a literal structural container; the `[listing]`
    // block style changes its context to `listing`. The default (raw) context
    // remains `literal`, and the resolved context is `listing`.
    let (raw, resolved, _, _) = facts("[listing]\n....\na > b\n....");
    assert_eq!(raw, "literal");
    assert_eq!(resolved, "listing");

    // The block renders as a listing block (not a literal block), with the
    // special characters in its verbatim content escaped.
    let doc = Parser::default().parse("[listing]\n....\na > b\n....");
    assert_css(&doc, "div.listingblock", 1);
    assert_css(&doc, "div.literalblock", 0);
    assert_xpath(
        &doc,
        "//div[@class=\"listingblock\"]//pre[contains(text(), \"a > b\")]",
        1,
    );
}

#[test]
fn transform_a_paragraph_into_another_block() {
    verifies!(
        r#"
The declared block style can also be used to transform a paragraph into a different kind of block.
The block will still retain the simple content model.
Let's consider the case of turning a normal paragraph into a sidebar.

----
[sidebar]
This sidebar is short, so a styled paragraph will do.
----

"#
    );

    // A `[sidebar]` style on a contiguous paragraph turns it into a sidebar that
    // still retains the simple content model.
    let (_, resolved, model, _) =
        facts("[sidebar]\nThis sidebar is short, so a styled paragraph will do.");
    assert_eq!(resolved, "sidebar");
    assert_eq!(model, ContentModel::Simple);

    // It renders as a sidebar block.
    let doc =
        Parser::default().parse("[sidebar]\nThis sidebar is short, so a styled paragraph will do.");
    assert_css(&doc, "div.sidebarblock > div.content", 1);
}

#[test]
fn admonition_masquerade_over_an_example_block() {
    verifies!(
        r#"
Finally, let's consider an admonition block.
Declaring the NOTE block style on an example structural container transforms it to an admonition block and also sets the style of the block to NOTE.

----
[NOTE]
====
Remember the milk.
====
----

"#
    );

    // The `NOTE` block style on a `====` example container transforms it into an
    // admonition block (context `admonition`). Because the example container has
    // the compound content model, the resulting admonition is also compound.
    let (raw, _, model, _) = facts("[NOTE]\n====\nRemember the milk.\n====");
    assert_eq!(raw, "admonition");
    assert_eq!(model, ContentModel::Compound);

    let doc = Parser::default().parse("[NOTE]\n====\nRemember the milk.\n====");
    assert_css(&doc, "div.admonitionblock.note", 1);
}

#[test]
fn admonition_masquerade_over_a_paragraph() {
    verifies!(
        r#"
This technique also works for converting a paragraph into an admonition block.

----
[NOTE]
Remember the milk.
----

"#
    );

    // The `NOTE` block style on a paragraph converts it into an admonition
    // block. Because the source is a paragraph, the admonition retains the
    // simple content model.
    let (raw, _, model, _) = facts("[NOTE]\nRemember the milk.");
    assert_eq!(raw, "admonition");
    assert_eq!(model, ContentModel::Simple);

    let doc = Parser::default().parse("[NOTE]\nRemember the milk.");
    assert_css(&doc, "div.admonitionblock.note", 1);
}

#[test]
fn specialize_change_or_both() {
    verifies!(
        r#"
Where permitted, the declared block style can be used to specialize the context of the block, change the context of the block, or both.

"#
    );

    // Specialize the context: `source` specializes the `listing` context.
    let (_, resolved, _, _) = facts("[source]\n....\na > b\n....");
    assert_eq!(resolved, "listing");

    // Change the context: `sidebar` changes a paragraph's context to `sidebar`.
    let (_, resolved, _, _) = facts("[sidebar]\nAn aside.");
    assert_eq!(resolved, "sidebar");

    // Both: `NOTE` changes the context (to `admonition`) and specializes it
    // (with the `NOTE` admonition style).
    let (raw, _, _, _) = facts("[NOTE]\nRemember the milk.");
    assert_eq!(raw, "admonition");
}

#[test]
fn built_in_permutations_intro() {
    non_normative!(
        r#"
== Built-in permutations

"#
    );

    verifies!(
        r#"
The table below lists the xref:delimited.adoc#table-of-structural-containers[structural containers] whose context can be altered, and which contexts are valid, using the declared block style.

"#
    );

    // The structural containers whose context can be altered are exercised
    // individually by `permutations_table` below; here we confirm one row (an
    // admonition masquerade over an example container) to anchor this section.
    let (raw, _, _, _) = facts("[NOTE]\n====\nx\n====");
    assert_eq!(raw, "admonition");
}

#[test]
fn permutations_table() {
    verifies!(
        r#"
[cols="1,1,3"]
|===
|Type |Default context |Masquerading contexts

|example
|:example
|admonition (designated by the NOTE, TIP, WARNING, CAUTION, or IMPORTANT style)

|listing
|:listing
|literal

|literal
|:literal
|listing (can be designated using the source style)

|open
|:open
|abstract, admonition (designated by the NOTE, TIP, WARNING, CAUTION, or IMPORTANT style), comment, example, literal, listing (can be designated using the source style), partintro, pass, quote, sidebar, verse

|pass
|:pass
|stem, latexmath, asciimath

|sidebar
|:sidebar
|_n/a_

|quote
|:quote
|verse
|===

"#
    );

    // example (default :example) -> admonition, for each admonition style.
    for style in ["NOTE", "TIP", "WARNING", "CAUTION", "IMPORTANT"] {
        let (raw, _, model, _) = facts(&format!("[{style}]\n====\nx\n===="));
        assert_eq!(
            raw, "admonition",
            "[{style}] over an example block should be an admonition"
        );
        assert_eq!(model, ContentModel::Compound);
    }

    // listing (default :listing) -> literal, via the `[literal]` style.
    let (raw, resolved, model, _) = facts("[literal]\n----\na > b\n----");
    assert_eq!(raw, "listing");
    assert_eq!(resolved, "literal");
    assert_eq!(model, ContentModel::Verbatim);

    // literal (default :literal) -> listing, via the `[listing]` style or the
    // `[source]` style.
    for style in ["listing", "source"] {
        let (raw, resolved, model, _) = facts(&format!("[{style}]\n....\na > b\n...."));
        assert_eq!(raw, "literal");
        assert_eq!(
            resolved, "listing",
            "[{style}] over a literal block should resolve to listing"
        );
        assert_eq!(model, ContentModel::Verbatim);
    }

    // open (default :open) -> a variety of contexts.
    //
    // admonition:
    let (raw, _, model, _) = facts("[NOTE]\n--\nx\n--");
    assert_eq!(raw, "admonition");
    assert_eq!(model, ContentModel::Compound);

    // example and sidebar (both compound, like open):
    for style in ["example", "sidebar"] {
        let (raw, resolved, model, _) = facts(&format!("[{style}]\n--\nx\n--"));
        assert_eq!(raw, "open");
        assert_eq!(
            resolved, style,
            "[{style}] over an open block should resolve to {style}"
        );
        assert_eq!(model, ContentModel::Compound);
    }

    // literal and listing (via the `source` style), both verbatim:
    let (_, resolved, model, _) = facts("[literal]\n--\nx\n--");
    assert_eq!(resolved, "literal");
    assert_eq!(model, ContentModel::Verbatim);
    for style in ["listing", "source"] {
        let (_, resolved, model, _) = facts(&format!("[{style}]\n--\nx\n--"));
        assert_eq!(resolved, "listing");
        assert_eq!(model, ContentModel::Verbatim);
    }

    // pass (raw, no substitutions, no nested-block parsing):
    let (raw, _, model, _) = facts("[pass]\n--\n<b>x</b>\n--");
    assert_eq!(raw, "pass");
    assert_eq!(model, ContentModel::Raw);

    // quote (compound) and verse (simple):
    let (raw, _, model, _) = facts("[quote]\n--\nx\n--");
    assert_eq!(raw, "quote");
    assert_eq!(model, ContentModel::Compound);
    let (raw, _, model, _) = facts("[verse]\n--\nx\n--");
    assert_eq!(raw, "verse");
    assert_eq!(model, ContentModel::Simple);

    // The `abstract`, `comment`, and `partintro` masquerades over an open block
    // are doctype- and converter-specific (e.g. `partintro` is valid only inside
    // a book part); the parser preserves the declared style for a downstream
    // converter to act on.
    for style in ["abstract", "comment", "partintro"] {
        let (_, _, _, declared) = facts(&format!("[{style}]\n--\nx\n--"));
        assert_eq!(declared.as_deref(), Some(style));
    }

    // pass (default :pass) -> stem, latexmath, asciimath. Each STEM style sets
    // the context to `stem` and the raw content model.
    for style in ["stem", "latexmath", "asciimath"] {
        let (raw, _, model, _) = facts(&format!("[{style}]\n++++\nx^2\n++++"));
        assert_eq!(
            raw, "stem",
            "[{style}] over a passthrough block should be a stem block"
        );
        assert_eq!(model, ContentModel::Raw);
    }

    // quote (default :quote) -> verse, via the `[verse]` style on a `____`
    // delimited block.
    let (raw, _, _, _) = facts("[verse]\n____\nThe fog comes\non little cat feet.\n____");
    assert_eq!(raw, "verse");
}

#[test]
fn paragraph_masquerades() {
    verifies!(
        r#"
All the contexts that can be applied to an open block can also be applied to a paragraph.
A paragraph also access the `normal` style, which can be applied to revert a literal paragraph to a normal paragraph.
"#
    );

    // The contexts that apply to an open block also apply to a paragraph; the
    // paragraph keeps the simple content model in each case.
    let (_, resolved, model, _) = facts("[sidebar]\nAn aside.");
    assert_eq!(resolved, "sidebar");
    assert_eq!(model, ContentModel::Simple);

    let (_, resolved, model, _) = facts("[example]\nAn example.");
    assert_eq!(resolved, "example");
    assert_eq!(model, ContentModel::Simple);

    let (raw, _, model, _) = facts("[quote]\nA quotation.");
    assert_eq!(raw, "quote");
    assert_eq!(model, ContentModel::Simple);

    let (raw, _, model, _) = facts("[verse]\nA verse.");
    assert_eq!(raw, "verse");
    assert_eq!(model, ContentModel::Simple);

    // The `normal` style reverts an (indentation-induced) literal paragraph back
    // to a normal paragraph: the content model stays simple, the context is
    // `paragraph`, and the block is parsed with the paragraph (not literal)
    // style.
    let doc =
        Parser::default().parse("[normal]\n  This would be a literal paragraph without the style.");
    let normal = doc.child_blocks().next().expect("expected a block");
    assert_eq!(normal.resolved_context().as_ref(), "paragraph");
    assert_eq!(normal.content_model(), ContentModel::Simple);
    if let crate::blocks::Block::Simple(simple) = normal {
        assert_eq!(simple.style(), SimpleBlockStyle::Paragraph);
    } else {
        panic!("expected a simple block, got {normal:?}");
    }

    // Without the `normal` style, the same indented text is a literal paragraph.
    let doc = Parser::default().parse("  This is a literal paragraph.");
    let literal = doc.child_blocks().next().expect("expected a block");
    if let crate::blocks::Block::Simple(simple) = literal {
        assert_eq!(simple.style(), SimpleBlockStyle::Literal);
    } else {
        panic!("expected a simple block, got {literal:?}");
    }
}
