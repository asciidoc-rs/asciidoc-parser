use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

track_file!("ref/asciidoc-lang/docs/modules/directives/pages/include-with-indent.adoc");

/// Include `content` into a listing block with the given include `attrs` and
/// return the resulting listing block source (with the `----` delimiters),
/// which reflects the indentation normalization applied by the preprocessor.
fn listing_include(attrs: &str, content: &'static str) -> String {
    let handler = InlineFileHandler::from_pairs([("code.rb", content)]);
    let source = format!("----\ninclude::code.rb[{attrs}]\n----");
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse(&source);
    doc.nested_blocks()
        .next()
        .unwrap()
        .span()
        .data()
        .to_string()
}

const INDENTED: &str = "    def names\n      @name.split ' '\n    end";

non_normative!(
    r#"
= Indent Included Content
// aka Normalize Block Indentation
// This content needs to be made applicable to includes...like add a step in the example flow to show it coming from an include file.

Source code snippets from external files are often padded with a leading block indent.
This leading block indent is relevant in its original context.
However, once inside the documentation, this leading block indent is no longer needed.

== The indent attribute

"#
);

#[test]
fn indent_behavior() {
    verifies!(
        r#"
The attribute `indent` allows the leading block indent to be stripped and, optionally, a new block indent to be set for blocks with verbatim content (listing, literal, source, verse, etc.).

* When `indent` is 0, the leading block indent is stripped
* When `indent` is > 0, the leading block indent is first stripped, then the content is indented by the number of columns equal to this value.

WARNING: If any line in the verbatim content is not indented, the `indent` attribute is effectively ignored.

"#
    );

    // `indent=0` strips the common leading block indent.
    assert_eq!(
        listing_include("indent=0", INDENTED),
        "----\ndef names\n  @name.split ' '\nend\n----"
    );

    // `indent=2` strips the leading block indent, then re-indents by 2 columns.
    assert_eq!(
        listing_include("indent=2", INDENTED),
        "----\n  def names\n    @name.split ' '\n  end\n----"
    );

    // A line with no indentation makes the common indent zero, so `indent` is
    // effectively ignored and the content is left unchanged.
    assert_eq!(
        listing_include("indent=4", "def names\n  @name.split ' '\nend"),
        "----\ndef names\n  @name.split ' '\nend\n----"
    );
}

#[test]
fn tabsize_expands_tabs() {
    // The parser expands tabs on the included content when `indent` is also
    // supplied; block-level tab expansion independent of `indent` is not yet
    // implemented, so the broader claim is tracked as a to-do.
    to_do_verifies!(
        r#"
If the `tabsize` attribute is set on the block or the document, tabs are also replaced with the number of spaces specified by that attribute, regardless of whether the `indent` attribute is set.

"#
    );

    let handler = InlineFileHandler::from_pairs([("code.rb", "\tdef names\n\t  @name\n\tend")]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute("tabsize", "4", ModificationContext::Anywhere)
        .with_include_file_handler(handler)
        .parse("----\ninclude::code.rb[indent=0]\n----");

    // Tabs are expanded to spaces (tab stop of 4), then the common indent is
    // stripped.
    assert_eq!(
        doc.nested_blocks().next().unwrap().span().data(),
        "----\ndef names\n  @name\nend\n----"
    );
}

non_normative!(
    r#"
Let's consider a source block that has included content which is indented.
Only the result of the include directive is shown here to help illustrate the behavior.
When the `indent` attribute is used on the source block in the following AsciiDoc source:

[source]
....
[source,ruby,indent=0]
----
    def names
      @name.split ' '
    end
----
....

The processor produces:

....
def names
  @name.split ' '
end
....

On the other hand, this AsciiDoc source:

[source]
....
[source,ruby,indent=2]
----
    def names
      @name.split ' '
    end
----
....

Produces:

----
  def names
    @name.split ' '
  end
----

"#
);

#[test]
fn positive_indent_strips_then_readds() {
    verifies!(
        r#"
Notice that when the `indent` attribute is positive, the block indentation is first removed, then readded using the specified amount.
"#
    );

    assert_eq!(
        listing_include("indent=2", INDENTED),
        "----\n  def names\n    @name.split ' '\n  end\n----"
    );
}
