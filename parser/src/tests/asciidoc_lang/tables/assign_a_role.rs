use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/assign-a-role.adoc");

/// Parse `source` as a single block and return the [`TableBlock`] it produced.
///
/// [`TableBlock`]: crate::blocks::TableBlock
fn parse_table(source: &str) -> crate::blocks::TableBlock<'_> {
    let mut parser = Parser::default();
    let mi = crate::blocks::Block::parse(crate::Span::new(source), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    match mi.item {
        crate::blocks::Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    }
}

// The `tag=base` table referenced by the example on the page: a minimal,
// two-cell table. Roles are independent of the table's shape, so this stands in
// for the included example.
const BASE: &str = "|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n|===";

non_normative!(
    r#"
= Assign a Role to a Table

"#
);

#[test]
fn role_attribute() {
    verifies!(
        r#"
Like with all blocks, you can add a role to a table using the `role` attribute.
"#
    );

    // The formal `role` named attribute assigns the role to the table.
    let src = format!("[role=name-of-role]\n{BASE}");
    let table = parse_table(&src);
    assert_eq!(table.roles(), vec!["name-of-role"]);

    // A role may also carry multiple, space-separated values.
    let src = format!("[role=\"role-one role-two\"]\n{BASE}");
    let table = parse_table(&src);
    assert_eq!(table.roles(), vec!["role-one", "role-two"]);

    // The mapping of a role to an HTML CSS class is a rendering concern; this
    // crate does not convert to HTML.
    non_normative!(
        r#"
The role attribute becomes a CSS class when converted to HTML.
"#
    );
}

#[test]
fn role_shorthand() {
    verifies!(
        r#"
The preferred shorthand for assigning the role attribute is to put the role name in the first position of the block attribute list prefixed with a `.` character, as shown here:

[source]
----
[.name-of-role]
include::example$table.adoc[tag=base]
----
"#
    );

    // The dot-prefixed shorthand in the first position assigns the role just
    // like the formal `role` attribute does.
    let src = format!("[.name-of-role]\n{BASE}");
    let table = parse_table(&src);
    assert_eq!(table.roles(), vec!["name-of-role"]);
}
