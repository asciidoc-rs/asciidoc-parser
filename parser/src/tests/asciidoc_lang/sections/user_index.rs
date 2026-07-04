use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/sections/pages/user-index.adoc");

non_normative!(
    r#"
= Index
:page-aliases: index.adoc

You can mark index terms explicitly in AsciiDoc content.
Index terms form a controlled vocabulary that can be used to navigate the document by keyword starting from an index.

"#
);

#[test]
fn index_catalog() {
    non_normative!(
        r#"
== Index catalog

NOTE: Although index terms are always processed, only Asciidoctor PDF and the DocBook toolchain support creating an index catalog automatically.
The built-in HTML5 converter in Asciidoctor does not generate an index.

"#
    );

    verifies!(
        r#"
To create an index, define a level 1 section (`==`) marked with the style `index` at the end of your document.
(In a multipart book, the index can be the last level 0 section (`=`)).

[source]
----
[index]
== Index
----

"#
    );

    // A level 1 section marked with the `index` style carries that style, which
    // a converter that generates an index uses as the seed section. (The level 0
    // multipart-book form is not modeled by this single-document crate.)
    let doc = Parser::default().parse("[index]\n== Index\n");
    let section = doc.nested_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("index"));
    assert_eq!(section.raw_context().as_ref(), "section");

    non_normative!(
        r#"
Both Asciidoctor PDF and the DocBook toolchain will automatically populate an index into this seed section.

The index will consist of term entries that link to (or otherwise cite) the location of each marked index term.
You learn how to mark an index term in the next section.

"#
    );
}

#[test]
fn index_terms() {
    non_normative!(
        r#"
== Index terms

Every index term, as well as every occurrence of that index term, must be explicitly marked in the AsciiDoc document.
It's not enough just to mark the first occurrence of an index term if you want every occurrence to appear in the index.
Instead, each occurrence you want to be cited in the index must be marked explicitly.

There are two types of index terms in AsciiDoc:

"#
    );

    verifies!(
        r#"
flow index term:: `\indexterm2:[<primary>]` +
`+((<primary>))+`
+
An index term that appears in the flow of text (i.e., a visible term) and in the index.
This type of index term can only be used to define a primary entry and is case sensitive.
If you want the entry to appear in the index using a different case, use an adjacent concealed index term, such as `+(((term)))Term+`.

"#
    );

    // A flow index term appears as a visible term in the flow of text. Both the
    // `indexterm2:[…]` macro form and the `((…))` shorthand render the primary
    // term inline.
    let doc = Parser::default().parse("indexterm2:[Lancelot] was a knight.");
    assert_eq!(rendered_paragraphs(&doc), &["Lancelot was a knight."]);

    let doc = Parser::default().parse("The ((Arthur)) was king.");
    assert_eq!(rendered_paragraphs(&doc), &["The Arthur was king."]);

    verifies!(
        r#"
concealed index term:: `\indexterm:[<primary>, <secondary>, <tertiary>]` +
`+(((<primary>, <secondary>, <tertiary>)))+`
+
A group of index terms that appear only in the index.
This type of index term can be used to define a primary entry as well as optional secondary and tertiary entries.

"#
    );

    // A concealed index term appears only in the index, never in the flow of
    // text, so both the `indexterm:[…]` macro form and the `(((…)))` shorthand
    // render as nothing.
    let doc = Parser::default().parse("indexterm:[knight, Knight of the Round Table, Lancelot]");
    assert_eq!(rendered_paragraphs(&doc), &[""]);

    let doc = Parser::default().parse("Excalibur(((Sword, Broadsword, Excalibur))).");
    assert_eq!(rendered_paragraphs(&doc), &["Excalibur."]);

    non_normative!(
        r#"
Here's an example that shows the two forms in use.

"#
    );

    verifies!(
        r#"
[source]
----
The Lady of the Lake, her arm clad in the purest shimmering samite,
held aloft Excalibur from the bosom of the water,
signifying by divine providence that I, ((Arthur)), <.>
was to carry Excalibur(((Sword, Broadsword, Excalibur))). <.>
That is why I am your king. Shut up! Will you shut up?!
Burn her anyway! I'm not a witch.
Look, my liege! We found them.

indexterm2:[Lancelot] was one of the Knights of the Round Table. <.>
indexterm:[knight, Knight of the Round Table, Lancelot] <.>
----
"#
    );

    // The callout markers (`<.>`) above are annotations on the listing, not part
    // of the marked-up content. Parsing the equivalent content, the visible
    // (flow) terms remain inline while the concealed terms disappear.
    let doc = Parser::default().parse(
        "signifying by divine providence that I, ((Arthur)),\nwas to carry Excalibur(((Sword, Broadsword, Excalibur))).",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        &["signifying by divine providence that I, Arthur,\nwas to carry Excalibur."]
    );

    verifies!(
        r#"
<.> The double parenthesis form adds a primary index term and includes the term in the generated output.
<.> The triple parenthesis form allows for an optional second and third index term and _does not_ include the terms in the generated output (i.e., concealed index term).
<.> The inline macro `\indexterm2:[primary]` is equivalent to the double parenthesis form.
<.> The inline macro `\indexterm:[primary, secondary, tertiary]` is equivalent to the triple parenthesis form.

"#
    );

    // The macro form and the parenthesis form are equivalent: `indexterm2:[…]`
    // matches `((…))` (visible) and `indexterm:[…]` matches `(((…)))`
    // (concealed).
    let macro_visible = Parser::default().parse("indexterm2:[Lancelot].");
    let shorthand_visible = Parser::default().parse("((Lancelot)).");
    assert_eq!(
        rendered_paragraphs(&macro_visible),
        rendered_paragraphs(&shorthand_visible)
    );

    let macro_concealed = Parser::default().parse("Knights.indexterm:[knight, Lancelot]");
    let shorthand_concealed = Parser::default().parse("Knights.(((knight, Lancelot)))");
    assert_eq!(
        rendered_paragraphs(&macro_concealed),
        rendered_paragraphs(&shorthand_concealed)
    );

    verifies!(
        r#"
If you're defining a concealed index term (i.e., the `indexterm` macro), and one of the terms contains a comma, you must surround that segment in double quotes so the comma is treated as content.
For example:

[source]
----
I, King Arthur.
indexterm:[knight, "Arthur, King"]
----

"#
    );

    // A comma inside a double-quoted term segment is treated as content, so the
    // concealed term parses cleanly and contributes nothing to the flow of text.
    let doc = Parser::default().parse("I, King Arthur.\nindexterm:[knight, \"Arthur, King\"]");
    assert_eq!(rendered_paragraphs(&doc), &["I, King Arthur.\n"]);

    verifies!(
        r#"
or

[source]
----
I, King Arthur.
(((knight, "Arthur, King")))
----

"#
    );

    // The shorthand concealed form quotes commas the same way.
    let doc = Parser::default().parse("I, King Arthur.\n(((knight, \"Arthur, King\")))");
    assert_eq!(rendered_paragraphs(&doc), &["I, King Arthur.\n"]);

    non_normative!(
        r#"
//Follow https://github.com/asciidoctor/asciidoctor/issues/450[Asciidoctor issue #450] to track the progress of this feature.

"#
    );
}

#[test]
fn placement_of_hidden_index_terms() {
    verifies!(
        r#"
== Placement of hidden index terms

Hidden index entries should be directly adjacent to the paragraph content to which they apply.
<<ex-hidden-terms-correct>> shows where to place hidden index terms for a paragraph.

.Correct
[#ex-hidden-terms-correct]
----
=== Create a new Git repository

(((Repository, create)))
(((Create Git repository)))
To create a new git repository,
----

"#
    );

    // When the hidden (concealed) terms are directly adjacent to the paragraph
    // they belong to, they are absorbed into that single paragraph (they render
    // as nothing), so the section contains exactly one paragraph.
    let doc = Parser::default().parse(
        "=== Create a new Git repository\n\n(((Repository, create)))\n(((Create Git repository)))\nTo create a new git repository,\n",
    );
    let section = doc.nested_blocks().next().unwrap();
    let paragraphs = rendered_paragraphs(&doc);
    assert_eq!(section.nested_blocks().count(), 1);
    assert_eq!(paragraphs, &["\n\nTo create a new git repository,"]);

    verifies!(
        r#"
If the terms are offset from the paragraph content by an empty line, it will cause an empty paragraph to be created in the parsed document, thus leaving extra space in the generated output.
<<ex-hidden-terms-incorrect-1>> and <<ex-hidden-terms-incorrect-2>> show where you should not place hidden index terms for a paragraph.

.Incorrect
[#ex-hidden-terms-incorrect-1]
----
=== Create a new Git repository

(((Repository, create)))
(((Create Git repository)))

To create a new git repository,
----

"#
    );

    // An empty line between the terms and the paragraph splits them into two
    // blocks: an (empty) paragraph holding only the now-invisible terms, and the
    // real paragraph.
    let doc = Parser::default().parse(
        "=== Create a new Git repository\n\n(((Repository, create)))\n(((Create Git repository)))\n\nTo create a new git repository,\n",
    );
    let section = doc.nested_blocks().next().unwrap();
    assert_eq!(section.nested_blocks().count(), 2);
    assert_eq!(
        rendered_paragraphs(&doc),
        &["\n", "To create a new git repository,"]
    );

    verifies!(
        r#"
.Also incorrect
[#ex-hidden-terms-incorrect-2]
----
=== Create a new Git repository
(((Repository, create)))
(((Create Git repository)))

To create a new git repository,
----
"#
    );

    // Likewise, terms stacked directly under the section title but separated
    // from the paragraph by an empty line form their own empty paragraph.
    let doc = Parser::default().parse(
        "=== Create a new Git repository\n(((Repository, create)))\n(((Create Git repository)))\n\nTo create a new git repository,\n",
    );
    let section = doc.nested_blocks().next().unwrap();
    assert_eq!(section.nested_blocks().count(), 2);
}
