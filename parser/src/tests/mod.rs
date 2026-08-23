#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

mod asciidoc_lang;
mod asciidoctor_rb;
pub(crate) mod assert_dom;
mod block_nesting_depth;
pub(crate) mod fixtures;
mod hash;
mod inline_builder_document_parity;
mod inline_builder_recorder_parity;
mod inline_recorder;
mod inline_substitution_renderer;
mod origin;
pub(crate) mod prelude;
pub(crate) mod sdd;
mod security;
mod table_cell_directive_warnings;
mod xref;
