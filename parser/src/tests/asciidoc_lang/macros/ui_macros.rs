use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/ui-macros.adoc");

non_normative!(
    r#"
= Button and Menu UI Macros

include::partial$ui-macros-disclaimer.adoc[]

"#
);

mod button_macro_syntax {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Button macro syntax

It can be difficult to communicate to the reader that they need to press a button.
They can't tell if you are saying "`OK`" or they are supposed to look for a button labeled *OK*.
It's all about getting the semantics right.
The `btn` macro to the rescue!

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
.Using the button macro syntax
[#ex-btn]
----
include::example$ui.adoc[tag=button]
----

The result of <<ex-btn>> is displayed below.

====
include::example$ui.adoc[tag=button]
====

"#
        );

        // The example above pulls in `example$ui.adoc[tag=button]`. The
        // spec-coverage tool can't follow includes, so the included content is
        // reproduced here and parsed directly (with `experimental` set, as the
        // UI macros require).
        let doc = Parser::default().parse(
            ":experimental:\n\nPress the btn:[OK] button when you are finished.\n\nSelect a file in the file navigator and click btn:[Open].",
        );
        assert_eq!(
            rendered_paragraphs(&doc),
            &[
                r#"Press the <b class="button">OK</b> button when you are finished."#,
                r#"Select a file in the file navigator and click <b class="button">Open</b>."#,
            ]
        );
    }
}

mod menu_macro_syntax {
    use crate::tests::prelude::*;

    fn render(input: &str) -> String {
        let doc = Parser::default().parse(&format!(":experimental:\n\n{input}"));
        rendered_paragraphs(&doc).join("")
    }

    non_normative!(
        r#"
== Menu macro syntax

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
Trying to explain how to select a menu item can be a pain.
With the `menu` macro, the symbols do the work.

.Using the menu macro syntax
[#ex-menu]
----
include::example$ui.adoc[tag=menu]
----

The instructions in <<ex-menu>> appear below.

====
include::example$ui.adoc[tag=menu]
====

"#
        );

        // The example above pulls in `example$ui.adoc[tag=menu]`. The
        // spec-coverage tool can't follow includes, so the included content is
        // reproduced here and parsed directly (with `experimental` set, as the
        // UI macros require). The two paragraphs exercise a bare menu item
        // (`menu:File[Save]`) and a submenu path (`menu:View[Zoom > Reset]`).
        let doc = Parser::default().parse(
            ":experimental:\n\nTo save the file, select menu:File[Save].\n\nSelect menu:View[Zoom > Reset] to reset the zoom level to the default setting.",
        );
        assert_eq!(
            rendered_paragraphs(&doc),
            &[
                r#"To save the file, select <span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save</b></span>."#,
                r#"Select <span class="menuseq"><b class="menu">View</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Zoom</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Reset</b></span> to reset the zoom level to the default setting."#,
            ]
        );
    }

    // The material below describes the *shorthand* menu syntax (`"File > Save"`).
    // This crate does not implement it and does not plan to: per the AsciiDoc
    // language documentation the shorthand is not on a standards track (see the
    // "No planned support" section of the crate README). These lines are
    // therefore kept non-normative — covered for completeness, but no behavior is
    // asserted.
    non_normative!(
        r#"
If the menu has more than one item, it can be expressed using a shorthand.

IMPORTANT: The shorthand syntax for menu is not on a standards track.
You can use it for transient documents, but do not rely on it long term.

In the shorthand syntax:

* each item is separated by a greater than sign (`>`) with spaces on either side
* the whole expression must be enclosed in double quotes (`"`)

The text of the item itself may contain spaces.

.Using the shorthand menu syntax
[#ex-menu-short]
----
include::example$ui.adoc[tag=menu-short]
----

The shorthand syntax can be escaped by preceding the opening double quote with a backslash character.

"#
    );

    #[test]
    fn first_menu_item_must_start_with_word_char_or_ampersand() {
        verifies!(
            r#"
Both the menu macro and menu shorthand require the first menu item start with a word character (alphanumeric character or underscore) or ampersand (to accommodate a character reference).
If you need the first menu item to start with a non-word character, you will need to substitute it with the equivalent character reference.
For example, to make a menu item that starts with vertical ellipsis, you must use `\&#8942;`.

"#
        );

        // The menu name may begin with an ampersand-introduced character
        // reference (here, a vertical ellipsis).
        assert_eq!(
            render("menu:&#8942;[More Tools, Extensions]"),
            r#"<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>"#
        );
        // A menu name that begins with a non-word, non-ampersand character is not
        // recognized as a menu macro, so it is emitted as literal text.
        assert_eq!(render("menu:.hidden[Item]"), "menu:.hidden[Item]");
    }

    #[test]
    fn subsequent_menu_items_may_start_with_any_character() {
        verifies!(
            r#"
.Using a character reference at the start of the menu
[#ex-menu-short-charref]
----
include::example$ui.adoc[tag=menu-charref]
----

Subsequent menu items don't have this requirement and thus can start with any character.
"#
        );

        // The example above (`example$ui.adoc[tag=menu-charref]`) uses the
        // *shorthand* menu syntax, which this crate does not implement (see the
        // note on the shorthand block above), so it is left as literal text
        // rather than converted to a menu. Starting the menu with a character
        // reference is verified for the macro form in
        // `first_menu_item_must_start_with_word_char_or_ampersand` above.
        assert_eq!(
            render(r#"Select "&#8942; > More Tools > Extensions" to find and enable extensions."#),
            r#"Select "&#8942; &gt; More Tools &gt; Extensions" to find and enable extensions."#
        );

        // Unlike the first menu item, subsequent items (submenus and the final
        // item) may start with any character, including a non-word character.
        assert_eq!(
            render("menu:View[.zoom > .reset]"),
            r#"<span class="menuseq"><b class="menu">View</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">.zoom</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">.reset</b></span>"#
        );
    }
}
