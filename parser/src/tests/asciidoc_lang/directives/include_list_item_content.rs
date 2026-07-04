use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

track_file!("docs/modules/directives/pages/include-list-item-content.adoc");

non_normative!(
    r#"
= Include List Item Content

"#
);

#[test]
fn empty_attribute_technique() {
    verifies!(
        r#"
You can use the include directive to include the content of a list item from another file, but there are some things you need to be aware of.

Recall that the `include` directive must be defined on a line by itself.
This presents a challenge with lists since each list item must begin with the list marker.
We can solve this by using the built-in `empty` attribute to initiate the list item, then follow that line with the include directive to bring in the actual content.

Here's an example of how to use the `empty` attribute and the include directive to define a list item, then include the primary text from another file:

"#
    );

    // The `empty` attribute begins the list item and the include directive on
    // the following line supplies the item's primary text.
    let handler = InlineFileHandler::from_pairs([("item-text.adoc", "The item text.")]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("* {empty}\ninclude::item-text.adoc[]");

    // The single list item's primary text comes from the included file.
    let paras = rendered_paragraphs(&doc);
    assert_eq!(paras.len(), 1);
    assert!(paras[0].contains("The item text."), "was: {paras:?}");
}

non_normative!(
    r#"
----
* {empty}
\include::item-text.adoc[]
----

"#
);

#[test]
fn open_block_technique() {
    verifies!(
        r#"
This technique works well if you control the contents of the included file and can ensure it only contains adjacent lines of text.
If a list item does not contain adjacent lines, the list may be terminated.
So we need a bit more syntax.

If you can't guarantee that all the included lines will be adjacent, you'll want to tuck the include directive inside an open block.
This keeps all the include lines together, enclosed inside the boundaries of the block.
You then attach this block to the list item using a list continuation (i.e., `+`).

Here's an example of how to include compound content from another file into a list item:

"#
    );

    // Wrapping the include in an open block attached with a list continuation
    // keeps compound content together within the list item.
    let handler = InlineFileHandler::from_pairs([(
        "complex-list-item.adoc",
        "First paragraph.\n\nSecond paragraph.",
    )]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("* {empty}\n+\n--\ninclude::complex-list-item.adoc[]\n--");

    let paras = rendered_paragraphs(&doc);
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["First paragraph.", "Second paragraph."]);
}

non_normative!(
    r#"
----
* {empty}
+
--
\include::complex-list-item.adoc[]
--
----

See xref:lists:continuation.adoc#drop-principal-text[dropping the principal text of a list item] for another example of this technique.
"#
);
