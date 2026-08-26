//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

mod content;
pub use content::Content;
pub(crate) use content::{
    DeferredParts, FootnoteDeferred, OwnedTitle, XrefSegment, fold_resolved_title,
    rehome_xref_placeholders, render_xref_template, resolved_destinations, sanitize_title,
    template_partition,
};
// The two tree walks behind `Content::set_tree_xrefs`, which calls them
// directly. Re-exported for the differential corpus that compares what they
// derive against the string pipeline's own answer; nothing else outside
// `content` reaches them.
#[cfg(test)]
pub(crate) use content::{block_tree_xref_segments, footnote_tree_xref_segments};

pub(crate) mod inline_builder;

// The Strategy-A recorder, retired from the production parse path when the
// single-pass builder (`inline_builder`) replaced it as `Content`'s tree
// source. It survives as test-only oracle machinery: the differential harness
// (`tests::inline_recorder`) still drives it directly, and the structural
// cross-check (`tests::inline_builder_recorder_parity`) compares its tree
// against the builder's.
#[cfg(test)]
pub(crate) mod inline_tree;

mod macros;
pub(crate) use macros::{
    INLINE_ANCHOR, INLINE_BIBLIO_ANCHOR, INLINE_EMAIL, INLINE_FOOTNOTE_MACRO, INLINE_IMAGE_MACRO,
    INLINE_INDEXTERM, INLINE_KBD_BTN_MACRO, INLINE_LINK, INLINE_LINK_MACRO, INLINE_MENU_MACRO,
    INLINE_XREF, NormalizedCaps, URI_SNIFF, basename, document_xrefstyle, encode_uri_component,
    extract_attributes_from_text, normalize_footnote_text, normalize_index_text,
    normalize_text_lf_escaped_bracket, split_kbd_keys, strip_see_and_seealso,
};

mod xref_target;

pub(crate) mod passthroughs;
pub use passthroughs::Passthrough;
pub(crate) use passthroughs::{
    INLINE_PASS, INLINE_PASS_MACRO, INLINE_STEM_MACRO, Passthroughs, stem_notation,
};

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    ATTRIBUTE_REFERENCE, AttributeMissing, CharacterReplacement, QuoteSub, build_callout_regexes,
    character_replacements, hard_line_break_pattern, maybe_has_quotes, maybe_has_replacements,
    quote_subs, restored_entity_pattern, substitute_attributes_in_macro_target,
    substitute_attributes_in_reftext,
};
