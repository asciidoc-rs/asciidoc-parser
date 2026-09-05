//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

mod content;
pub use content::Content;
pub(crate) use content::{
    DeferredParts, FootnoteDeferred, OwnedTitle, XrefSegment, XrefTemplatePiece,
    fold_resolved_title, render_xref_template, resolved_destinations, sanitize_title,
};
use content::{render_xref_segment, xref_segment_from_node};

pub(crate) mod inline_builder;

mod macros;
use macros::{
    INLINE_ANCHOR, INLINE_BIBLIO_ANCHOR, INLINE_EMAIL, INLINE_FOOTNOTE_MACRO, INLINE_IMAGE_MACRO,
    INLINE_INDEXTERM, INLINE_KBD_BTN_MACRO, INLINE_LINK, INLINE_LINK_MACRO, INLINE_MENU_MACRO,
    INLINE_XREF, URI_SNIFF, basename, document_xrefstyle, encode_uri_component,
    extract_attributes_from_text, normalize_index_text, normalize_text_lf_escaped_bracket,
    split_kbd_keys,
};

mod xref_target;

pub(crate) mod passthroughs;
pub use passthroughs::Passthrough;
use passthroughs::{INLINE_PASS, INLINE_PASS_MACRO, INLINE_STEM_MACRO, stem_notation};

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
use substitution_step::ATTRIBUTE_REFERENCE;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    AttributeMissing, apply_attributes, apply_special_characters,
    substitute_attributes_in_macro_target, substitute_attributes_in_reftext,
};
