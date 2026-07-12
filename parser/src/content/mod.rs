//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

mod content;
pub use content::Content;
pub(crate) use content::{
    FOOTNOTE_MARKER_END, FOOTNOTE_MARKER_START, FootnoteDeferred, XrefSegment,
    rehome_xref_placeholders, strip_footnote_marker_spans,
};

mod macros;

pub(crate) mod passthroughs;
pub(crate) use passthroughs::Passthroughs;

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    substitute_attributes_in_macro_target, substitute_attributes_in_text,
};
