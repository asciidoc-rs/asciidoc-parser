//! This module quotes from the AsciiDoc language documentation
//! with the intent of verifying, line-by-line, compliance with
//! that documentation.
//!
//! This documentation is not considered by its authors to be a formal language
//! specification (and I agree), but it's as close as is available at this time.
//!
//! The quoted documentation can be found in rendered form here:
//! https://docs.asciidoctor.org/asciidoc/latest/
//!
//! and in source form here:
//! https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/tree/main/docs/modules
//!
//! This documentation comes with the following license information:
//!
//! > AsciiDoc documentation (c) by Sarah White, Dan Allen, and other
//! > contributors.
//!
//! > Content under this directory is made available under the terms of a
//! > Creative Commons Attribution 4.0 International License.
//! > You can find the complete text of this license at https://creativecommons.org/licenses/by/4.0/.
//!
//! The submodules in this module are named to match the
//! page structure as rendered on the AsciiDoctor site.

mod attributes;
mod blocks;
mod directives;
mod document;
mod lists;
mod macros;
mod pass;
mod root;
mod sections;
mod subs;
mod text;
mod verbatim;
