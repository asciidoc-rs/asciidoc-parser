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
mod inline_builder_passthrough_record_parity;
mod inline_builder_side_effect_parity;
mod inline_renderer;
mod inline_tree;
mod origin;
mod parse_termination;
pub(crate) mod prelude;
pub(crate) mod sdd;
mod security;
mod sentinels;
mod table_cell_directive_warnings;
mod xref;
