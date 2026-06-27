//! Generic block captioning and numbering.
//!
//! Some block contexts are *captionable*: when such a block has a title, it is
//! given a caption that prepends a label and an automatically incremented
//! number to that title (e.g. an example block titled "Onomatopoeia" renders
//! "Example 1. Onomatopoeia"). This mirrors Asciidoctor, where the behavior is
//! granted not by a dedicated block type but by the block's context appearing
//! in a lookup table that maps it to the document attribute supplying its
//! label.
//!
//! The caption and the bare number are computed once, during parsing, and
//! stored as first-class members of the block (see [`IsBlock::caption`] and
//! [`IsBlock::number`]); the converter simply concatenates the caption prefix
//! and the title. Numbers are assigned in document order as blocks finish
//! parsing, so a nested captioned block is numbered before its container.
//!
//! [`IsBlock::caption`]: crate::blocks::IsBlock::caption
//! [`IsBlock::number`]: crate::blocks::IsBlock::number

use crate::{
    Parser, attributes::Attrlist, blocks::is_built_in_context, document::InterpretedValue,
};

/// Maps a block context to the document attribute that supplies its caption
/// label, or `None` if the context is not captionable.
///
/// This mirrors Asciidoctor's `CAPTION_ATTRIBUTE_NAMES`. The label attribute
/// for each context defaults to a locale-specific value (e.g. `example-caption`
/// defaults to "Example", `table-caption` to "Table"); unsetting it disables
/// numbering for that context.
pub(crate) fn caption_attribute_name(context: &str) -> Option<&'static str> {
    match context {
        "example" => Some("example-caption"),
        "figure" => Some("figure-caption"),
        "listing" => Some("listing-caption"),
        "table" => Some("table-caption"),
        _ => None,
    }
}

/// The caption prefix and bare number assigned to a block.
///
/// The `prefix` is the text prepended to the block's title (e.g. `"Example 1.
/// "`, including the trailing separator and space). The `number` is the bare
/// counter value (e.g. `1`); it is `None` when an explicit caption override is
/// in effect, since overrides are not numbered.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Caption {
    /// The caption prefix prepended to the title (e.g. `"Example 1. "`).
    pub(crate) prefix: String,

    /// The bare counter value, or `None` for an explicit (unnumbered) caption.
    pub(crate) number: Option<usize>,
}

/// Computes the caption for a block from its raw context and attribute list.
///
/// This is the entry point used while parsing a block. It resolves the
/// captioning context (a block style naming a built-in context, such as
/// `[example]` over an open block, takes precedence over the raw context —
/// mirroring [`resolved_context`]) and the explicit caption override, then
/// delegates to [`assign_caption`]:
///
/// * a `[caption=...]` attribute supplies a verbatim, unnumbered override;
/// * a collapsible example has its caption suppressed (kept unnumbered),
///   exactly as Asciidoctor's parser does.
///
/// [`resolved_context`]: crate::blocks::IsBlock::resolved_context
pub(crate) fn assign_block_caption(
    parser: &mut Parser,
    raw_context: &str,
    attrlist: Option<&Attrlist<'_>>,
    has_title: bool,
) -> Option<Caption> {
    let resolved_context = attrlist
        .and_then(|attrlist| attrlist.block_style())
        .filter(|style| is_built_in_context(style))
        .unwrap_or(raw_context);

    let explicit_caption =
        if let Some(caption) = attrlist.and_then(|attrlist| attrlist.named_attribute("caption")) {
            Some(caption.value().to_string())
        } else if resolved_context == "example"
            && attrlist.is_some_and(|attrlist| attrlist.has_option("collapsible"))
        {
            Some(String::new())
        } else {
            None
        };

    assign_caption(
        parser,
        resolved_context,
        has_title,
        explicit_caption.as_deref(),
    )
}

/// Computes the caption for a block, following Asciidoctor's `assign_caption`.
///
/// Returns `None` (no caption, no number) when:
/// * the block has no title (`has_title` is `false`) — untitled blocks are
///   never captioned or counted;
/// * an explicit caption override is present but empty (e.g. `[caption=]`, or a
///   collapsible example whose caption is suppressed); or
/// * the context is not captionable, or its caption attribute is unset.
///
/// When a non-empty explicit `caption` override is supplied (from a
/// `[caption=]` attribute) — or the document-wide `caption` attribute is set —
/// that value is used verbatim as the prefix, with **no** number assigned.
/// Otherwise the label comes from the context's caption attribute and the
/// document-wide counter for that context is incremented, producing a `"<label>
/// <n>. "` prefix.
pub(crate) fn assign_caption(
    parser: &mut Parser,
    context: &str,
    has_title: bool,
    explicit_caption: Option<&str>,
) -> Option<Caption> {
    // An untitled block is never captioned or counted. Likewise, captioning
    // applies only to captionable contexts: this gate matches Asciidoctor, where
    // `assign_caption` is only invoked when the block's context is a key in
    // `CAPTION_ATTRIBUTE_NAMES`, so an override such as a document-wide `caption`
    // attribute never leaks onto, say, an ordinary titled paragraph.
    if !has_title {
        return None;
    }
    let attr_name = caption_attribute_name(context)?;

    // An explicit caption override (from `[caption=...]`) wins and is never
    // numbered. An explicitly empty override suppresses the caption entirely
    // (this is how a collapsible example is kept unnumbered).
    if let Some(value) = explicit_caption {
        return if value.is_empty() {
            None
        } else {
            Some(Caption {
                prefix: value.to_string(),
                number: None,
            })
        };
    }

    // A document-wide `caption` attribute, if set, overrides the per-context
    // label and likewise skips the counter.
    if let InterpretedValue::Value(value) = parser.attribute_value("caption")
        && !value.is_empty()
    {
        return Some(Caption {
            prefix: value,
            number: None,
        });
    }

    // Otherwise build "<label> <n>. " from the context's caption attribute,
    // consuming the next value of that context's document-wide counter. When the
    // caption attribute is unset (or empty), the block keeps its title but
    // receives no caption and no number.
    match parser.attribute_value(attr_name) {
        InterpretedValue::Value(label) if !label.is_empty() => {
            let number = parser.increment_counter(&format!("{context}-number"));
            Some(Caption {
                prefix: format!("{label} {number}. "),
                number: Some(number),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use crate::{blocks::IsBlock, tests::prelude::*};

    /// Returns the (caption, number) of the first nested block in `input`.
    fn first_block_caption(input: &str) -> (Option<String>, Option<usize>) {
        let doc = Parser::default().parse(input);
        let block = doc.nested_blocks().next().expect("expected a block");
        (block.caption().map(str::to_string), block.number())
    }

    #[test]
    fn caption_attribute_name_lookup() {
        assert_eq!(
            super::caption_attribute_name("example"),
            Some("example-caption")
        );
        assert_eq!(
            super::caption_attribute_name("figure"),
            Some("figure-caption")
        );
        assert_eq!(
            super::caption_attribute_name("listing"),
            Some("listing-caption")
        );
        assert_eq!(
            super::caption_attribute_name("table"),
            Some("table-caption")
        );
        assert_eq!(super::caption_attribute_name("sidebar"), None);
        assert_eq!(super::caption_attribute_name("paragraph"), None);
    }

    #[test]
    fn untitled_block_is_not_captioned() {
        assert_eq!(first_block_caption("====\nbody.\n===="), (None, None));
    }

    #[test]
    fn titled_example_is_numbered_by_default() {
        assert_eq!(
            first_block_caption(".Title\n====\nbody.\n===="),
            (Some("Example 1. ".to_string()), Some(1))
        );
    }

    #[test]
    fn counter_is_per_context_and_document_wide() {
        // Two titled examples share one sequence; an untitled example between
        // them consumes no number.
        let doc =
            Parser::default().parse(".One\n====\na\n====\n\n====\nb\n====\n\n.Two\n====\nc\n====");
        let numbers: Vec<_> = doc.nested_blocks().map(|b| b.number()).collect();
        assert_eq!(numbers, vec![Some(1), None, Some(2)]);
    }

    #[test]
    fn custom_label_from_caption_attribute() {
        assert_eq!(
            first_block_caption(":example-caption: Exhibit\n\n.Title\n====\nbody.\n===="),
            (Some("Exhibit 1. ".to_string()), Some(1))
        );
    }

    #[test]
    fn unset_caption_attribute_disables_numbering() {
        assert_eq!(
            first_block_caption(":!example-caption:\n\n.Title\n====\nbody.\n===="),
            (None, None)
        );
    }

    #[test]
    fn explicit_caption_attribute_is_verbatim_and_unnumbered() {
        assert_eq!(
            first_block_caption("[caption=\"Sample: \"]\n.Title\n====\nbody.\n===="),
            (Some("Sample: ".to_string()), None)
        );
    }

    #[test]
    fn empty_explicit_caption_suppresses_caption() {
        assert_eq!(
            first_block_caption("[caption=]\n.Title\n====\nbody.\n===="),
            (None, None)
        );
    }

    #[test]
    fn document_caption_attribute_overrides_label() {
        // A document-wide `caption` attribute supplies a verbatim, unnumbered
        // label for captionable blocks...
        assert_eq!(
            first_block_caption(":caption: Sample\n\n.Title\n====\nbody.\n===="),
            (Some("Sample".to_string()), None)
        );

        // ...but it must not leak onto a non-captionable context such as an
        // ordinary titled paragraph.
        assert_eq!(
            first_block_caption(":caption: Sample\n\n.Title\nplain paragraph."),
            (None, None)
        );
    }

    #[test]
    fn collapsible_example_is_unnumbered() {
        // A collapsible example suppresses its caption (and number), and does not
        // consume a counter value, so a following example is still "Example 1".
        let doc = Parser::default()
            .parse("[%collapsible]\n====\nhidden\n====\n\n.Title\n====\nbody.\n====");
        let numbers: Vec<_> = doc.nested_blocks().map(|b| b.number()).collect();
        assert_eq!(numbers, vec![None, Some(1)]);
    }
}
