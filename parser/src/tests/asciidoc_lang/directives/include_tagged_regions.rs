use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

track_file!("docs/modules/directives/pages/include-tagged-regions.adoc");

/// Include `file` into a listing block selecting tags with `spec`, returning
/// the selected lines (joined by `\n`, without the surrounding `----`
/// delimiters).
fn tag_select(file: &'static str, spec: &str) -> String {
    let handler = InlineFileHandler::from_pairs([("f.adoc", file)]);
    let source = format!("----\ninclude::f.adoc[{spec}]\n----");
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse(&source);
    let span = doc
        .nested_blocks()
        .next()
        .unwrap()
        .span()
        .data()
        .to_string();
    span.strip_prefix("----\n")
        .and_then(|s| s.strip_suffix("\n----"))
        .map(str::to_string)
        .unwrap_or(span)
}

// A region `a` containing a nested region `b`, plus a trailing untagged line.
const NESTED: &str =
    "// tag::a[]\nalpha\n// tag::b[]\nbeta\n// end::b[]\ngamma\n// end::a[]\ndelta";

// A region `foo` containing a nested region `bar`, with untagged lines on
// either side. Used for the tag-filtering permutations.
const FOOBAR: &str =
    "before\n// tag::foo[]\nfoo1\n// tag::bar[]\nbar1\n// end::bar[]\nfoo2\n// end::foo[]\nafter";

non_normative!(
    r#"
= Include Content by Tagged Regions
// aka Select Portions of a Document to Include

"#
);

#[test]
fn overview() {
    verifies!(
        r#"
The include directive enables you to select portions of a file to include instead of including the whole file.
Use the `lines` attribute to include individual lines or a range of lines (by line number).
Use the `tags` attribute (or `tag` attribute for the singular case) to select lines that fall between regions marked with user-defined tags.

When including multiple line ranges or multiple tags, each entry in the list must be separated by either a comma or a semi-colon.
If commas are used, the entire value must be enclosed in quotes (per attribute rules).
You can eliminate the need for quotes by using the semi-colon as the data separator instead.

"#
    );

    // Two separate regions selected with a semicolon-delimited list of tags...
    let separate = "// tag::a[]\naaa\n// end::a[]\nmid\n// tag::b[]\nbbb\n// end::b[]";
    assert_eq!(tag_select(separate, "tags=a;b"), "aaa\nbbb");
    // ...and with a (quoted) comma-delimited list.
    assert_eq!(tag_select(separate, "tags=\"a,b\""), "aaa\nbbb");
}

non_normative!(
    r#"
== Tagging regions

"#
);

#[test]
fn tag_directive_syntax() {
    verifies!(
        r#"
Tags are useful when you want to identify specific regions of a file to include.
You can then select the lines between the boundaries of the include tag/end directives to include using the `tags` attribute.

In the include file, the tag directives (e.g., `tag::name[]` and `end::name[]`) must follow a word boundary and precede a space character or the end of line.
The tag name must not be empty and must consist exclusively of non-space characters.

"#
    );

    // A `tag` selects the lines between its `tag::name[]` and `end::name[]`
    // boundaries; the directive lines themselves are discarded.
    assert_eq!(tag_select(NESTED, "tag=a"), "alpha\nbeta\ngamma");

    // A `tag::` that does not follow a word boundary is not a directive, so the
    // line is treated as ordinary content.
    let not_a_directive = "// tag::a[]\nfootag::x[]\n// end::a[]";
    assert_eq!(tag_select(not_a_directive, "tag=a"), "footag::x[]");
}

non_normative!(
    r#"
Typically, the tag directives will be placed after a line comment as defined by the language of the source file.
This is especially important when using tags to include portions of an AsciiDoc document which may itself be converted.
If the tag is not behind a line comment, the tag will be treated as regular content, which could disrupt the block structure, such as a table.

For languages that only support circumfix comments, such as XML, you can enclose the tag directives between the circumfix comment markers, offset by a space on either side.
For example, in XML files, you can use `+<!-- tag::name[] -->+` and `+<!-- end::name[] -->+`.

"#
);

#[test]
fn tag_includes_all_matching_regions() {
    verifies!(
        r#"
Including by tag includes all regions marked with that tag.
This makes it possible to include a group of lines from different regions of the document using a single tag.

"#
    );

    // The same tag name used on two regions selects the lines from both.
    let two_regions = "// tag::t[]\nfirst\n// end::t[]\nskip\n// tag::t[]\nsecond\n// end::t[]";
    assert_eq!(tag_select(two_regions, "tag=t"), "first\nsecond");
}

non_normative!(
    r#"
TIP: If the target file has tagged lines, and you just want to ignore those lines, use the `tags` attribute to filter them out.
See <<tag-filtering>> for details.

The example below shows how you tag a region of content inside a file containing multiple code examples.
The tag directives are preceded by a hash (`#`) because that's the start of a line comment in Ruby.

.Tagged code snippets in a file named core.rb
[source,ruby,subs=attributes+]
----
include::example$include.adoc[tag=tag-co]
----
<.> To indicate the start of a tagged region, insert a comment line in the code.
<.> Assign a name to the `tag` directive. In this example, the tag is named _timings_.
<.> Insert another comment line where you want the tagged region to end.
<.> Assign the name of the region you want to terminate to the `end` directive.
<.> This is the start of a tagged snippet named _parse_.
<.> This is the end of the tagged snippet named _parse_.

In the next example, the tagged region named _parse_ is selected by the `include` directive.

.Selecting the _parse_ code snippet from a document
[source]
....
include::example$include.adoc[tag=target-co]
....
<.> In the directive's brackets, set the `tag` attribute and assign it the unique name of the code snippet you tagged in your code file.

You can include multiple tags from the same file.

.Selecting the _timings_ and the _parse_ code snippets from a document
[source]
....
include::example$include.adoc[tag=target-co-multiple]
....

It's also possible to have fine-grained tagged regions inside larger tagged regions.

In the next example, tagged regions are defined behind line comments.
By putting each tag behind a line comment, regardless of how the content is included, you don't have to worry about those lines appearing in the rendered document.

----
// tag::snippets[]
// tag::snippet-a[]
snippet a
// end::snippet-a[]

// tag::snippet-b[]
snippet b
// end::snippet-b[]
// end::snippets[]
----

Let's assume you include this file using the following include directive:

----
\include::file-with-snippets.adoc[tag=snippets]
----

The following lines will be selected and displayed:

....
snippet a

snippet b
....

You could also include the whole file without worry that the tags will be rendered:

----
\include::file-with-snippets.adoc[]
----

Now let's consider the case when the tags are not placed behind line comments.
In this case, you need to ensure that xref:tag-filtering[tag filtering] is being used or else those tags will be visible in the rendered document.

----
text a

tag::snippet-b[]
snippet b
end::snippet-b[]

text c
----

If you only want to include a specific tagged region of the file, use the following include directive:

----
\include::file-with-snippets.adoc[tag=snippet-b]
----

The following lines will be selected and displayed:

....
snippet b
....

If you want to include the whole file, but also filter out any include tags, use the following include directive:

----
\include::file-with-snippets.adoc[tag=**]
----

The following lines will be selected and displayed:

....
text a
snippet b
text c
....

If you did not specify tag filtering, tag directives that aren't behind a line comment (e.g., `tag::snippet-b[]`) will also be printed too.
Tag filtering is explained in more detail in the next section.

[#tag-filtering]
== Tag filtering

The previous section showed how to select tagged regions explicitly, but you can also use wildcards and exclusions.
These expressions give you the ability to include or exclude tags in bulk.
For example, here's how to include all lines that are not enclosed in a tag:

----
\include::file-with-snippets.adoc[tag=!*]
----

"#
);

#[test]
fn tag_directive_lines_discarded() {
    verifies!(
        r#"
When tag filtering is used, lines that contain a tag directive _are always discarded_ (like a line comment).
Even if you're not including content by tags, you can specify the double wildcard (`+**+`) to filter out all lines in the include file that contain a tag directive.

"#
    );

    // `**` keeps every line except the tag-directive lines.
    assert_eq!(
        tag_select(FOOBAR, "tags=**"),
        "before\nfoo1\nbar1\nfoo2\nafter"
    );
}

non_normative!(
    r#"
The modifiers you can use for filtering are as follows:

"#
);

#[test]
fn filtering_modifiers() {
    verifies!(
        r#"
`*`::
The single wildcard.
Select all tagged regions.
May only be specified once, negated or not.

`**`::
The double wildcard.
Select all the lines in the document *except for lines that contain a tag directive*.
Use this symbol if you want to include a file that has tag directives, but you want to discard the lines that contain a tag directive.
May only be specified once, negated or not.

`!`::
Negate the wildcard or tag.

"#
    );

    // Single wildcard: all tagged regions (nested included), no untagged lines.
    assert_eq!(tag_select(FOOBAR, "tags=*"), "foo1\nbar1\nfoo2");
    // Double wildcard: every line except tag-directive lines.
    assert_eq!(
        tag_select(FOOBAR, "tags=**"),
        "before\nfoo1\nbar1\nfoo2\nafter"
    );
    // Negation: everything except the named region.
    assert_eq!(tag_select(FOOBAR, "tags=!foo"), "before\nafter");
}

non_normative!(
    r#"
The double wildcard is always applied first, regardless of where it appears in the list.
If the double wildcard is not negated (i.e., `+**+`), it should only be combined with exclusions (e.g., `+**;!foo+`).
A negated double wildcard (i.e., `+!**+`), which selects no lines, is usually implied as the starting point.
A negated single wildcard has different meaning depending on whether it comes before tag names (e.g., `+!*;foo+`) or after at least one tag name (e.g., `+foo;!*+`).

Let's assume we have a region tagged `foo` with a nested region tagged `bar`.
Here are some of the permutations you can use (along with their implied long-hand forms):

"#
);

#[test]
fn filtering_permutations() {
    verifies!(
        r#"
`+**+`:: Selects all the lines in the document (except for lines that contain a tag directive).
_(implies `+**;*+`)_

`+*+`:: Selects all tagged regions in the document.
Does not select lines outside of tagged regions.
_(implies `+!**;*+`)_

`+!*+`:: Selects only the regions in the document outside of tags (i.e., non-tagged regions).
_(implies `+**;!*+`)_

`foo`:: Selects only regions tagged _foo_.
_(implies `+!**;foo+`)_

`foo;!bar`:: Selects only regions tagged _foo_, but excludes any nested regions tagged _bar_.
_(implies `+!**;foo;!bar+`)_

`+foo;!*+`:: Selects only regions tagged _foo_, but excludes any nested tagged regions.
_(implies `+!**;foo;!*+`)_

`+*;!foo+`:: Selects all tagged regions, but excludes any regions tagged _foo_ (nested or otherwise).
_(implies `+!**;*;!foo+`)_

`!foo`:: Selects all the lines in the document except for regions tagged _foo_.
_(implies `+**;!foo+`)_

`!foo;!bar`:: Selects all the lines in the document except for regions tagged _foo_ or _bar_.
_(implies `+**;!foo;!bar+`)_

`+!*;foo+`:: Selects the regions in the document outside of tags (i.e., non-tagged regions) and inside regions tagged _foo_, excluding any nested tagged regions.
To include nested tagged regions, they each must be named explicitly.
_(implies `+**;!*;foo+`)_

"#
    );

    // `**`: all non-directive lines.
    assert_eq!(
        tag_select(FOOBAR, "tags=**"),
        "before\nfoo1\nbar1\nfoo2\nafter"
    );
    // `*`: all tagged regions, no untagged lines.
    assert_eq!(tag_select(FOOBAR, "tags=*"), "foo1\nbar1\nfoo2");
    // `!*`: only the untagged regions.
    assert_eq!(tag_select(FOOBAR, "tags=!*"), "before\nafter");
    // `foo`: the foo region, including its nested bar region.
    assert_eq!(tag_select(FOOBAR, "tags=foo"), "foo1\nbar1\nfoo2");
    // `foo;!bar`: the foo region, excluding the nested bar region.
    assert_eq!(tag_select(FOOBAR, "tags=foo;!bar"), "foo1\nfoo2");
    // `foo;!*`: the foo region, excluding any nested tagged region.
    assert_eq!(tag_select(FOOBAR, "tags=foo;!*"), "foo1\nfoo2");
    // `!foo`: everything except the foo region.
    assert_eq!(tag_select(FOOBAR, "tags=!foo"), "before\nafter");
}

#[test]
fn implicit_base_selection() {
    verifies!(
        r#"
If the filter begins with a negated tag or single wildcard, it implies that the pattern begins with `+**+`.
An exclusion not preceded by an inclusion implicitly starts by selecting all the lines that do not contain a tag directive.
Otherwise, it implies that the pattern begins with `+!**+`.
A leading inclusion implicitly starts by selecting no lines.
"#
    );

    // A leading exclusion (`!foo`) implies a `**` base: untagged lines are kept.
    assert_eq!(tag_select(FOOBAR, "tags=!foo"), "before\nafter");
    // A leading inclusion (`foo`) implies a `!**` base: untagged lines dropped.
    assert_eq!(tag_select(FOOBAR, "tags=foo"), "foo1\nbar1\nfoo2");
}
