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
