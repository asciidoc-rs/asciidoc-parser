//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

mod content;
pub use content::Content;
pub(crate) use content::{
    FOOTNOTE_MARKER_END, FOOTNOTE_MARKER_START, FootnoteDeferred, OwnedTitle,
    PASSTHROUGH_PLACEHOLDER_END, PASSTHROUGH_PLACEHOLDER_START, XrefSegment, escape_sentinels,
    rehome_xref_placeholders, render_xref_template, sanitize_title, strip_footnote_marker_spans,
    unescape_sentinels,
};

mod macros;
pub(crate) use macros::apply_macros_with_leading_anchor_registered;

mod xref_target;

pub(crate) mod passthroughs;
pub use passthroughs::Passthrough;
pub(crate) use passthroughs::Passthroughs;

mod substitution_group;
pub use substitution_group::SubstitutionGroup;

mod substitution_step;
pub use substitution_step::SubstitutionStep;
pub(crate) use substitution_step::{
    AttributeMissing, substitute_attributes_in_macro_target, substitute_attributes_in_reftext,
};
