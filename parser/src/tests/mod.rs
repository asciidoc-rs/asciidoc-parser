#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

mod asciidoc_lang;
mod asciidoctor_rb;
pub(crate) mod assert_dom;
pub(crate) mod fixtures;
pub(crate) mod prelude;
pub(crate) mod sdd;
mod table_cell_directive_warnings;
mod xref;

/// The version of (Ruby) Asciidoctor that this crate is tested against.
///
/// The reference test suite vendored in `ref/asciidoctor` is taken from this
/// release, and any test that needs to supply Asciidoctor's own
/// `asciidoctor-version` attribute should use this constant rather than
/// spelling the version out. Update it here – and re-vendor `ref/asciidoctor` –
/// when the crate starts tracking a newer Asciidoctor release.
pub(crate) const ASCIIDOCTOR_VERSION: &str = "2.0.26";
