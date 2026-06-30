use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/counters.adoc");

/// Renders `input` and returns the rendered content of each of its top-level
/// blocks, in document order. A counter example is self-contained (it resets
/// the counter with a leading attribute entry), so this is enough to observe
/// the sequence it produces.
fn rendered_blocks(input: &str) -> Vec<String> {
    let doc = Parser::default().parse(input);
    doc.nested_blocks()
        .filter_map(|b| b.rendered_content().map(str::to_string))
        .collect()
}

non_normative!(
    r#"
= Counters
// document attributes and counters are NOT the same thing, but modifying a document attribute with the same name as the counter modifies the counter at the same time.

Counters are used to store and display ad-hoc sequences of numbers or Latin characters.

WARNING: Counters are a poorly defined feature in AsciiDoc and should be avoided if possible.
If you do use counters, you should only used them for the most rudimentary use cases, such as making a sequence in a list, table column, or prose.
You should *not* use counters to build IDs (i.e., references) or reference text.
Using counters across the boundaries of a reference will very likely result in unexpected behavior.

"#
);

#[test]
fn declare_and_display_a_counter() {
    verifies!(
        r#"
A counter is implemented as a specialized document attribute.
You declare and display a counter using an attribute reference, where the attribute name is prefixed with `counter:` (e.g., `+{counter:name}+`).
Since counters are attributes, counter names follow the same rules as xref:names-and-values.adoc#user-defined[attribute names].
The most important rule to note is that letters in counter names _must be lowercase_.

The counter value is incremented and displayed every time the `counter:` attribute reference is resolved.
The term [.term]*increment* means to advance the attribute value to the next value in the sequence.
If the counter value is an integer, add 1.
If the counter value is a character, move to the next letter in the Latin alphabet (e.g., a -> b).
The default start value of a counter is 1.

To create a sequence starting at 1, use the simple form `+{counter:name}+` as shown here:

[source]
The salad calls for {counter:seq1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears.

Here's the resulting output:

====
:!seq1:
The salad calls for {counter:seq1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears.
====

"#
    );

    assert_eq!(
        rendered_blocks(
            ":!seq1:\nThe salad calls for {counter:seq1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears."
        ),
        vec!["The salad calls for 1) apples, 2) oranges and 3) pears.".to_string()]
    );
}

#[test]
fn use_a_counter_in_a_section_title() {
    verifies!(
        r#"
If you want to use a counter value in a section title, you should define it first using an attribute reference.

----
:seq1: {counter:seq1}
== Section {seq1}

The sequence in this section is {seq1}.

:seq1: {counter:seq1}
== Section {seq1}

The sequence in this section is {seq1}.
----

Here's the resulting output:

====
:!seq1:

:seq1: {counter:seq1}
[discrete]
== Section {seq1}

The sequence in this section is {seq1}.

:seq1: {counter:seq1}
[discrete]
== Section {seq1}

The sequence in this section is {seq1}.
====

"#
    );

    let doc = Parser::default().parse(
        ":!seq1:\n\n:seq1: {counter:seq1}\n[discrete]\n== Section {seq1}\n\nThe sequence in this section is {seq1}.\n\n:seq1: {counter:seq1}\n[discrete]\n== Section {seq1}\n\nThe sequence in this section is {seq1}.",
    );

    assert_xpath(&doc, "//h2[text()=\"Section 1\"]", 1);
    assert_xpath(&doc, "//h2[text()=\"Section 2\"]", 1);
    assert_output_contains(&doc, "The sequence in this section is 1.");
    assert_output_contains(&doc, "The sequence in this section is 2.");
}

#[test]
fn counter2_increments_without_displaying() {
    verifies!(
        r#"
To increment the counter without displaying it (i.e., to skip an item in the sequence), use the `counter2` prefix instead:

[source]
{counter2:seq1}

WARNING: A `counter2` attribute reference on a line by itself will produce an empty paragraph.
You'll need to adjoin it to the nearest content to avoid this side effect.

"#
    );

    // A `counter2` reference advances the counter but renders nothing, so a
    // reference on a line by itself yields an (empty) paragraph; a later plain
    // reference shows the value it advanced to.
    assert_eq!(
        rendered_blocks("{counter2:seq1}\n\n{seq1}"),
        vec![String::new(), "1".to_string()]
    );
}

#[test]
fn display_the_current_value_without_incrementing() {
    verifies!(
        r#"
To display the current value of the counter without incrementing it, reference the counter name as you would any other attribute:

[source]
{counter2:pnum}This is paragraph {pnum}.

"#
    );

    // `counter2:pnum` advances `pnum` to 1 without displaying it; the plain
    // `{pnum}` reference then displays the current value without advancing it.
    assert_eq!(
        rendered_blocks("{counter2:pnum}This is paragraph {pnum}."),
        vec!["This is paragraph 1.".to_string()]
    );
}

#[test]
fn create_a_character_sequence_or_custom_start_value() {
    verifies!(
        r#"
To create a character sequence, or start a number sequence with a value other than 1, specify a start value by appending it to the first use of the counter:

[source]
Dessert calls for {counter:seq1:A}) mangoes, {counter:seq1}) grapes and {counter:seq1}) cherries.

CAUTION: Character sequences either run from a,b,c,...x,y,z,{,|... or A,B,C,...,X,Y,Z,[,... depending on the start value.
Therefore, they aren't really useful for more than 26 items.

"#
    );

    assert_eq!(
        rendered_blocks(
            "Dessert calls for {counter:seq1:A}) mangoes, {counter:seq1}) grapes and {counter:seq1}) cherries."
        ),
        vec!["Dessert calls for A) mangoes, B) grapes and C) cherries.".to_string()]
    );
}

#[test]
fn reset_a_counter_by_unsetting_the_attribute() {
    verifies!(
        r#"
The start value of a counter is only recognized if the counter is _unset_ at that point in the document.
Otherwise, the start value is ignored.

To reset a counter attribute, unset the corresponding attribute using an attribute entry.
The attribute entry must be adjacent to a block or else it is ignored.

[source]
----
The salad calls for {counter:seq1:1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears.

:!seq1:
Dessert calls for {counter:seq1:A}) mangoes, {counter:seq1}) grapes and {counter:seq1}) cherries.
----

This gives:

====
:!seq1:
The salad calls for {counter:seq1:1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears.

:!seq1:
Dessert calls for {counter:seq1:A}) mangoes, {counter:seq1}) grapes and {counter:seq1}) cherries.
====

"#
    );

    assert_eq!(
        rendered_blocks(
            ":!seq1:\nThe salad calls for {counter:seq1:1}) apples, {counter:seq1}) oranges and {counter:seq1}) pears.\n\n:!seq1:\nDessert calls for {counter:seq1:A}) mangoes, {counter:seq1}) grapes and {counter:seq1}) cherries."
        ),
        vec![
            "The salad calls for 1) apples, 2) oranges and 3) pears.".to_string(),
            "Dessert calls for A) mangoes, B) grapes and C) cherries.".to_string(),
        ]
    );

    // The start value is ignored once the counter is set: re-seeding `seq1`
    // with a different value mid-sequence has no effect.
    assert_eq!(
        rendered_blocks("{counter:seq1:5} {counter:seq1:9} {counter:seq1:9}"),
        vec!["5 6 7".to_string()]
    );
}

#[test]
fn use_a_counter_for_part_numbers_in_a_table() {
    verifies!(
        r#"
Here's a full example that shows how to use a counter for part numbers in a table.

[source]
----
include::example$counter.adoc[tag=base]
----

Here's the output of that table:

====
include::example$counter.adoc[tag=base]
====
"#
    );

    // The body of `example$counter.adoc` (tag `base`), inlined here so the test
    // does not depend on include resolution.
    let doc = Parser::default().parse(
        ".Parts{counter2:index:0}\n|===\n|Part Id |Description\n\n|PX-{counter:index}\n|Description of PX-{index}\n\n|PX-{counter:index}\n|Description of PX-{index}\n|===",
    );

    // The table itself is numbered with the document-wide `table-number`
    // counter, while the per-part `index` counter is seeded to 0 (without being
    // displayed) by the title and then advances 1, 2 across the rows.
    assert_xpath(&doc, "//caption[text()=\"Table 1. Parts\"]", 1);
    assert_xpath(&doc, "//p[text()=\"PX-1\"]", 1);
    assert_xpath(&doc, "//p[text()=\"Description of PX-1\"]", 1);
    assert_xpath(&doc, "//p[text()=\"PX-2\"]", 1);
    assert_xpath(&doc, "//p[text()=\"Description of PX-2\"]", 1);
}
