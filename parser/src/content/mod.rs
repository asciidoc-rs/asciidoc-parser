//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

mod content;
pub use content::Content;
pub(crate) use content::{
    FOOTNOTE_MARKER_END, FOOTNOTE_MARKER_START, FootnoteDeferred, OwnedTitle, XrefSegment,
    block_tree_xrefs, footnote_tree_xrefs, rehome_xref_placeholders, render_xref_template,
    strip_footnote_marker_spans,
};

pub(crate) mod inline_builder;

pub(crate) mod inline_tree;

mod macros;
pub(crate) use macros::{
    INLINE_ANCHOR, INLINE_FOOTNOTE_MACRO, INLINE_IMAGE_MACRO, INLINE_INDEXTERM,
    INLINE_KBD_BTN_MACRO, INLINE_LINK, INLINE_LINK_MACRO, INLINE_MENU_MACRO, INLINE_XREF,
    NormalizedCaps, URI_SNIFF, apply_macros_with_leading_anchor_registered, basename,
    normalize_footnote_text, normalize_index_text, normalize_text_lf_escaped_bracket,
    split_kbd_keys, strip_see_and_seealso,
};

mod xref_target;

pub(crate) mod passthroughs;
pub use passthroughs::Passthrough;
pub(crate) use passthroughs::{INLINE_PASS_MACRO, Passthroughs};

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    ATTRIBUTE_REFERENCE, AttributeMissing, CharacterReplacement, QuoteSub, character_replacements,
    hard_line_break_pattern, maybe_has_quotes, maybe_has_replacements, quote_subs,
    substitute_attributes_in_macro_target, substitute_attributes_in_reftext,
    substitute_attributes_in_text,
};
