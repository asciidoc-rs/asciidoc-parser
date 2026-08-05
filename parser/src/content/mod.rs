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
    INLINE_IMAGE_MACRO, INLINE_KBD_BTN_MACRO, INLINE_MENU_MACRO,
    apply_macros_with_leading_anchor_registered, basename, normalize_index_text,
    normalize_text_lf_escaped_bracket, split_kbd_keys,
};

mod xref_target;

pub(crate) mod passthroughs;
pub use passthroughs::Passthrough;
pub(crate) use passthroughs::Passthroughs;

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    AttributeMissing, CharacterReplacement, QuoteSub, character_replacements,
    hard_line_break_pattern, maybe_has_quotes, maybe_has_replacements, quote_subs,
    substitute_attributes_in_macro_target, substitute_attributes_in_reftext,
    substitute_attributes_in_text,
};
