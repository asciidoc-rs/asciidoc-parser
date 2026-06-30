use crate::{blocks::Block, tests::prelude::*};

track_file!("docs/modules/lists/pages/checklist.adoc");

non_normative!(
    r#"
= Checklists

List items can be marked complete using checklists.

"#
);

/// Returns the first block of `doc` as a [`ListBlock`], panicking if it is not
/// a list.
///
/// [`ListBlock`]: crate::blocks::ListBlock
fn first_list<'a>(doc: &'a crate::Document<'a>) -> &'a crate::blocks::ListBlock<'a> {
    match doc.nested_blocks().next() {
        Some(Block::List(list)) => list,
        other => panic!("expected a list block, got {other:?}"),
    }
}

#[test]
fn checklist_syntax() {
    verifies!(
        r#"
Checklists (i.e., task lists) are unordered lists that have items marked as checked (`[*]` or `[x]`) or unchecked (`[ ]`).
Here's an example:

.Checklist syntax
[#ex-syntax]
----
include::example$checklist.adoc[tag=check]
----

The result of <<ex-syntax>> is displayed below.

include::example$checklist.adoc[tag=check]

TIP: Not all items in the list have to be checklist items, as <<ex-syntax>> shows.

"#
    );

    // The example referenced above (`example$checklist.adoc[tag=check]`).
    let doc = Parser::default()
        .parse("* [*] checked\n* [x] also checked\n* [ ] not checked\n* normal list item\n");

    let checklist = first_list(&doc);
    assert!(checklist.is_checklist());

    // `[*]` and `[x]` are checked; `[ ]` is unchecked; an item with no checkbox
    // syntax (the TIP: not every item has to be a checklist item) carries no
    // checkbox state.
    let items: Vec<_> = checklist.nested_blocks().collect();
    assert_eq!(items[0].as_list_item().unwrap().checkbox(), Some(true));
    assert_eq!(items[1].as_list_item().unwrap().checkbox(), Some(true));
    assert_eq!(items[2].as_list_item().unwrap().checkbox(), Some(false));
    assert_eq!(items[3].as_list_item().unwrap().checkbox(), None);

    // The checklist renders with the `checklist` class, and each checkbox is
    // used in place of the item's bullet (a check mark for checked items, a
    // ballot box for unchecked ones).
    assert_css(&doc, ".ulist.checklist", 1);
    assert_xpath(
        &doc,
        "(/*[@class=\"ulist checklist\"]/ul/li)[1]/p[text()=\"\u{2713} checked\"]",
        1,
    );
    assert_xpath(
        &doc,
        "(/*[@class=\"ulist checklist\"]/ul/li)[3]/p[text()=\"\u{274f} not checked\"]",
        1,
    );
    assert_xpath(
        &doc,
        "(/*[@class=\"ulist checklist\"]/ul/li)[4]/p[text()=\"normal list item\"]",
        1,
    );
}

// The HTML rendering details below describe Asciidoctor's HTML5 converter
// output (the precise `data-item-complete` markup, and the `disabled` mark for
// static output). The interactive behavior is exercised by
// `interactive_checklist` below; the static-vs-interactive HTML specifics are a
// converter concern.
non_normative!(
    r#"
When checklists are converted to HTML, the checkbox markup is transformed into an HTML checkbox with the appropriate checked state.
The `data-item-complete` attribute on the checkbox is set to 1 if the item is checked, 0 if not.
The checkbox is used in place of the item's bullet.

Since HTML generated from AsciiDoc is typically static, the checkbox is set as disabled to make it appear as a simple mark.
If you want to make the checkbox interactive (i.e., clickable), add the `interactive` option to the checklist (shown here using the shorthand syntax for the xref:attributes:options.adoc[]):

"#
);

#[test]
fn interactive_checklist() {
    verifies!(
        r#"
.Checklist with interactive checkboxes
[#ex-interactive]
----
include::example$checklist.adoc[tag=check-int]
----

The result of <<ex-interactive>> is displayed below.

include::example$checklist.adoc[tag=check-int]

"#
    );

    // The example referenced above (`example$checklist.adoc[tag=check-int]`).
    let doc = Parser::default().parse(
        "[%interactive]\n* [*] checked\n* [x] also checked\n* [ ] not checked\n* normal list item\n",
    );

    let checklist = first_list(&doc);
    assert!(checklist.is_checklist());
    assert!(checklist.has_option("interactive"));

    // Each checklist item renders an interactive `<input type="checkbox">`. The
    // `data-item-complete` attribute is 1 (with `checked`) for checked items and
    // 0 for unchecked ones; no checkbox is disabled.
    assert_css(&doc, ".ulist.checklist", 1);
    assert_css(&doc, ".ulist.checklist li input[type=\"checkbox\"]", 3);
    assert_css(
        &doc,
        ".ulist.checklist li input[type=\"checkbox\"][checked]",
        2,
    );
    assert_css(
        &doc,
        ".ulist.checklist li input[type=\"checkbox\"][disabled]",
        0,
    );
}

// The remainder of the page is an authoring comment block (`////` … `////`)
// describing a possible font-based-icon example. Comment blocks are not part of
// the rendered output, so this is non-normative.
non_normative!(
    r#"
////
This example doesn't seem quite right since nothing about it indicates font based icons.

As a bonus, if you enable font-based icons, the checkbox markup (in non-interactive lists) is transformed into a font-based icon!

.Checklist with font-based checkboxes
[source]
----
include::{partialsdir}/ex-ulist.adoc[tag=check-icon]
----
////
"#
);
