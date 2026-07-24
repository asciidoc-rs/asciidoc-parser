#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

mod asciidoc_lang;
mod asciidoctor_rb;
pub(crate) mod assert_dom;
mod block_nesting_depth;
pub(crate) mod fixtures;
pub(crate) mod prelude;
pub(crate) mod sdd;
mod security;
mod table_cell_directive_warnings;
mod xref;
