use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/customize-title-label.adoc");

non_normative!(
    r#"
= Customize the Title Label
:table-caption: Data Set

When you xref:add-title.adoc[add a title to a table], the processor automatically prefixes it with the label _Table <n>._, where _<n>_ is the 1-based index of all of the titled tables in the document.
This label can be modified at the document level or per table.
It can also be xref:turn-off-title-label.adoc[deactivated].

"#
);

#[test]
fn modify_the_label_using_table_caption() {
    non_normative!(
        r#"
== Modify the label using table-caption

"#
    );

    verifies!(
        r#"
You can change the label for all titled tables using the document attribute `table-caption`.
(Don't let the attribute's name mislead you.
It's the attribute that controls the table title labels at the document level.)

In the document header, set the `table-caption` attribute and assign it your custom label text.

[source]
----
= Document Title
:table-caption: Data Set <.> <.>
----
<.> Set the document attribute `table-caption` and assign it the text you want to precede each table title.
<.> Don't enter a number after the label text.
The processor will automatically insert and increment the number.

In <<ex-label>>, the first and third tables have a title, but the second table doesn't have a title.

.Add two titled tables and one untitled table to a document
[source#ex-label]
----
= Document Title
:table-caption: Data Set

.A table with a title
[cols="2,1"]
|===
|Lots and lots of data |A little data

|834,734 |3
|3,999,271.5601 |5
|===

|===
|Group |Climate |Example

|A
|Tropical
|Suva, Fiji

|B
|Arid
|Lima, Peru
|===

.Another table with a title
|===
|Value |Result |Notes

|Null |A mystery |See Appendix R
|===
----

Since `table-caption` is assigned the value `Data Set`, any table title should be preceded with the label _Data Set <n>._
The three tables from <<ex-label>> are displayed below.

.A table with a title
[cols="2,1"]
|===
|Lots and lots of data |A little data

|834,734 |3
|3,999,271.5601 |5
|===

|===
|Group |Climate |Example

|A
|Tropical
|Suva, Fiji

|B
|Arid
|Lima, Peru
|===

.Another table with a title
|===
|Value |Result |Notes

|Null |A mystery |See Appendix R
|===

Notice that the table that doesn't have a title didn't get a label nor was it counted when the processor incremented the label number.
Therefore, the third table is assigned the label _Data Set 2._

"#
    );

    // The three tables share a document whose `table-caption` is "Data Set". The
    // two titled tables are labeled "Data Set 1." and "Data Set 2."; the untitled
    // middle table is neither labeled nor counted, so the third table is numbered
    // 2 rather than 3.
    let doc = Parser::default().parse(
        "= Document Title\n:table-caption: Data Set\n\n.A table with a title\n[cols=\"2,1\"]\n|===\n|Lots and lots of data |A little data\n\n|834,734 |3\n|3,999,271.5601 |5\n|===\n\n|===\n|Group |Climate |Example\n\n|A\n|Tropical\n|Suva, Fiji\n\n|B\n|Arid\n|Lima, Peru\n|===\n\n.Another table with a title\n|===\n|Value |Result |Notes\n\n|Null |A mystery |See Appendix R\n|===",
    );

    // The displayed tables render the label and title together inside each
    // titled table's `<caption class="title">`. The untitled middle table emits
    // no caption, so only two captions exist in the document.
    assert_xpath(
        &doc,
        "//caption[text()=\"Data Set 1. A table with a title\"]",
        1,
    );
    assert_xpath(
        &doc,
        "//caption[text()=\"Data Set 2. Another table with a title\"]",
        1,
    );
    assert_xpath(&doc, "//caption", 2);
}

#[test]
fn modify_the_label_of_an_individual_table_using_caption() {
    non_normative!(
        r#"
== Modify the label of an individual table using caption

"#
    );

    verifies!(
        r#"
You can customize the label on an individual table by setting the `caption` attribute.
(Don't let the name of the attribute mislead you.
The caption attribute only sets the caption's label, not the whole caption line).
When using `caption`, assign it the exact value you want displayed (including trailing spaces).
Labels assigned using `caption` don't get an automatically incremented number and only apply to the table they are set on.

CAUTION: If you want a space between the label and the title, you must add a trailing space to the value of the caption attribute.

.Modify the label using caption
[source#ex-caption]
----
[caption="Table A. "] <.> <.>
.A table with a custom label
[cols="3*"]
|===
|Null
|A mystery
|See Appendix R
|===
----
<.> Create an attribute list directly above the table's title and set the named attribute `caption`, followed by an equals sign (`=`), and then a value.
<.> Enclose the value in double quotation marks (`"`).
Otherwise the processor will remove any trailing whitespaces, and the title text will start directly after the last character of the label.

The table from <<ex-caption>> is displayed below.

[caption="Table A. "]
.A table with a custom label
[cols="3*"]
|===
|Null
|A mystery
|See Appendix R
|===

"#
    );

    // The explicit `caption` attribute (on its own attribute-list line above the
    // title, with the `cols` specifier on a second line below it) sets the label
    // verbatim — including its trailing space — with no automatically inserted
    // number. The label and title render together inside the table's
    // `<caption class="title">`.
    let doc = Parser::default().parse(
        "[caption=\"Table A. \"]\n.A table with a custom label\n[cols=\"3*\"]\n|===\n|Null\n|A mystery\n|See Appendix R\n|===",
    );

    assert_xpath(
        &doc,
        "//caption[text()=\"Table A. A table with a custom label\"]",
        1,
    );

    verifies!(
        r#"
If you create any subsequent tables in your document and don't set `caption` on them, the title labels will revert to the value assigned to `table-caption`.

"#
    );

    // A table that sets `caption` is skipped by the table counter, so a later
    // titled table without `caption` reverts to the `table-caption` label and
    // resumes the automatic numbering from "Table 1.".
    let doc = Parser::default()
        .parse("[caption=\"Custom. \"]\n.Custom\n|===\n|a\n|===\n\n.Next\n|===\n|b\n|===");

    assert_xpath(&doc, "//caption[text()=\"Custom. Custom\"]", 1);
    assert_xpath(&doc, "//caption[text()=\"Table 1. Next\"]", 1);

    non_normative!(
        r#"
If you want the caption of the table to only consist of the caption label, use the following syntax:

"#
    );

    // Reproducing this example requires two features the parser does not yet
    // support: setting a block's title through a `title` attribute, and the
    // `{counter:table-number}` counter reference. The example is tracked here
    // but not yet exercised.
    //
    // TODO: Promote to `verifies!` once counter support lands.
    // https://github.com/asciidoc-rs/asciidoc-parser/issues/514
    to_do_verifies!(
        r#"
[source]
----
[caption=,title="{table-caption} {counter:table-number}"]
include::example$table.adoc[tag=b-col-h]
----

"#
    );

    if false {
        todo!("block title attribute and counter:table-number support");
    }
}

#[test]
fn modify_the_label_of_an_individual_table_to_only_the_caption_label() {
    non_normative!(
        r#"
Alternately, you can write is as follows:

"#
    );

    // Reproducing this example requires `{counter:table-number}` counter
    // support, which is not yet implemented.
    //
    // TODO: Promote to `verifies!` once counter support lands.
    // https://github.com/asciidoc-rs/asciidoc-parser/issues/514
    to_do_verifies!(
        r#"
[source]
----
.{empty}
[caption="{table-caption} {counter:table-number}"]
include::example$table.adoc[tag=b-col-h]
----
"#
    );

    if false {
        todo!("counter:table-number support");
    }
}
