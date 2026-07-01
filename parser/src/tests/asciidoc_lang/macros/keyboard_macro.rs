use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/keyboard-macro.adoc");

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

    non_normative!(
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
}
