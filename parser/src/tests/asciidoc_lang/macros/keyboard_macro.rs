use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/keyboard-macro.adoc");

non_normative!(
    r#"
= Keyboard Macro

The keyboard macro allows to create a reference to a key or key sequence on a keyboard.
You can use this macro when you need to communicate to a reader what key or key sequence to press to perform a function.

include::partial$ui-macros-disclaimer.adoc[]

"#
);

mod keyboard_macro_syntax {
    use crate::tests::prelude::*;

    fn render(input: &str) -> String {
        let doc = Parser::default().parse(&format!(":experimental:\n\n{input}"));
        rendered_paragraphs(&doc).join("")
    }

    /// Returns the rendered content of the first ("Shortcut") column of every
    /// body row of the first table in `doc`.
    fn shortcut_cells(doc: &crate::Document<'_>) -> Vec<String> {
        use crate::blocks::{Block, TableCellContent};

        let mut out = vec![];
        for block in doc.nested_blocks() {
            if let Block::Table(table) = block {
                for row in table.body_rows() {
                    if let Some(cell) = row.cells().first()
                        && let TableCellContent::Simple(content) = cell.content()
                    {
                        out.push(content.rendered().to_string());
                    }
                }
            }
        }
        out
    }

    non_normative!(
        r#"
== Keyboard macro syntax

"#
    );

    #[test]
    fn separators_and_key_display() {
        verifies!(
            r#"
The keyboard macro uses the short (no target) macro syntax `+kbd:[key(+key)*]+`.
Each key is displayed as entered in the document.
Multiple keys are separated by a plus (e.g., `Ctrl+T`) or a comma (e.g., `Ctrl,T`).
The plus is preferred.

"#
        );

        // Plus and comma separators are equivalent, and each key is displayed
        // exactly as entered, wrapped in a key sequence.
        let with_plus = render("kbd:[Ctrl+T]");
        assert_eq!(
            with_plus,
            r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>T</kbd></span>"#
        );
        assert_eq!(render("kbd:[Ctrl,T]"), with_plus);
    }

    #[test]
    fn uppercase_is_customary_but_not_enforced() {
        verifies!(
            r#"
It's customary to represent alpha keys in uppercase, though this is not enforced.

"#
        );

        // Case is preserved exactly as entered; a lowercase key is not upcased.
        assert_eq!(render("kbd:[f3]"), "<kbd>f3</kbd>");
    }

    #[test]
    fn backslash_and_bracket_escaping() {
        verifies!(
            r#"
If the last key is a backslash (`\`), it must be followed by a space.
Without this space, the processor will not recognize the macro.
If one of the keys is a closing square bracket (`]`), it must be preceded by a backslash.
Without the backslash escape, the macro will end prematurely.
You can find example of these cases in the example below.

"#
        );

        // A trailing backslash key must be followed by a space so it is not
        // mistaken for an escape...
        assert_eq!(render("kbd:[\\ ]"), "<kbd>\\</kbd>");
        // ...and a closing square bracket key must be escaped with a backslash so
        // the macro does not end prematurely.
        assert_eq!(
            render("kbd:[Ctrl+\\]]"),
            r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>]</kbd></span>"#
        );
    }

    #[test]
    fn example_table() {
        verifies!(
            r#"
.Using the keyboard macro syntax
[#ex-kbd]
----
include::example$ui.adoc[tag=key]
----

The result of <<ex-kbd>> is displayed below.

[%autowidth]
include::example$ui.adoc[tag=key]
"#
        );

        // The example above pulls in `example$ui.adoc[tag=key]`. The spec-coverage
        // tool can't follow includes, so the included table is reproduced here and
        // parsed directly (with `experimental` set, as the UI macros require). Each
        // `Shortcut` cell exercises a documented case: a lone key, key sequences,
        // the backslash-plus-space escape, and the escaped closing bracket.
        let input = format!(
            ":experimental:\n\n{}",
            r#"|===
|Shortcut |Purpose

|kbd:[F11]
|Toggle fullscreen

|kbd:[Ctrl+T]
|Open a new tab

|kbd:[Ctrl+Shift+N]
|New incognito window

|kbd:[\ ]
|Used to escape characters

|kbd:[Ctrl+\]]
|Jump to keyword

|kbd:[Ctrl + +]
|Increase zoom
|==="#
        );
        let doc = Parser::default().parse(&input);

        assert_eq!(
            shortcut_cells(&doc),
            &[
                "<kbd>F11</kbd>",
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>T</kbd></span>"#,
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>N</kbd></span>"#,
                r#"<kbd>\</kbd>"#,
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>]</kbd></span>"#,
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd></span>"#,
            ]
        );
    }
}
