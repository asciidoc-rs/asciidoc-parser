use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/tables/pages/turn-off-title-label.adoc");

non_normative!(
    r#"
= Turn Off the Title Label
:table-caption!:

"#
);

#[test]
fn disable_the_label_using_table_caption() {
    non_normative!(
        r#"
== Disable the label using table-caption

"#
    );

    verifies!(
        r#"
You can disable the title label for all of the tables in a document by unsetting the `table-caption` document attribute.

[source]
----
= Title of Document
:table-caption!: <.>

.A table with a title but no label
|===
|Value |Result |Notes

|Null |A mystery |See Appendix R
|===
----
<.> In an attribute entry, enter the name of the attribute, `table-caption`, and append a bang (`!`) to the end of the name.
This unsets the attribute.

When `table-caption` is unset, table titles aren't preceded by a label and label number.

.A table with a title but no label
|===
|Value |Result |Notes

|Null |A mystery |See Appendix R
|===

"#
    );

    // Unsetting `table-caption` in the document header suppresses the label for
    // every titled table: the title still renders inside the table's
    // `<caption class="title">`, but with no "Table <n>." prefix in front of it.
    let doc = Parser::default().parse(
        "= Title of Document\n:table-caption!:\n\n.A table with a title but no label\n|===\n|Value |Result |Notes\n\n|Null |A mystery |See Appendix R\n|===",
    );

    assert_xpath(
        &doc,
        "//caption[text()=\"A table with a title but no label\"]",
        1,
    );
    assert_xpath(&doc, "//caption", 1);
}

#[test]
fn disable_the_label_using_caption() {
    non_normative!(
        r#"
== Disable the label using caption

"#
    );

    verifies!(
        r#"
To remove the label on an individual table, assign an empty value to the `caption` attribute.

[source]
----
[caption=] <.>
.A table with a title but no label
[cols="2,1"]
|===
|Lots and lots of data |A little data

|834,734 |3
|3,999,271.5601 |5
|===
----
<.> Enter the attribute's name, `caption`, in an attribute list directly above the table title, followed by an equals sign (`=`).
Don't enter a value after the `=`.

The table from the previous example is displayed below.

[caption=]
.A table with a title but no label
[cols="2,1"]
|===
|Lots and lots of data |A little data

|834,734 |3
|3,999,271.5601 |5
|===
"#
    );

    // An empty `caption` attribute removes the label on this individual table
    // only: the title renders inside the table's `<caption class="title">` with
    // no "Table <n>." prefix, and the table is skipped by the document-wide
    // counter rather than consuming a number.
    let doc = Parser::default().parse(
        "[caption=]\n.A table with a title but no label\n[cols=\"2,1\"]\n|===\n|Lots and lots of data |A little data\n\n|834,734 |3\n|3,999,271.5601 |5\n|===",
    );

    assert_xpath(
        &doc,
        "//caption[text()=\"A table with a title but no label\"]",
        1,
    );
    assert_xpath(&doc, "//caption", 1);
}
