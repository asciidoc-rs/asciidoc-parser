//! The [`Parser`] struct and its related structs allow a caller to configure
//! how AsciiDoc parsing occurs and then to initiate the parsing process.

mod attribute_value;
pub(crate) use attribute_value::AttributeValue;
pub use attribute_value::{AllowableValue, ModificationContext};

mod built_in_attrs;

mod docinfo_file_handler;
pub use docinfo_file_handler::DocinfoFileHandler;

mod include_file_handler;
pub use include_file_handler::IncludeFileHandler;

mod inline_substitution_renderer;
pub use inline_substitution_renderer::{
    CalloutGuard, CalloutRenderParams, CharacterReplacementType, FootnoteRenderParams,
    HtmlSubstitutionRenderer, IconRenderParams, ImageRenderParams, IndexTermRenderParams,
    InlineSubstitutionRenderer, LinkRenderParams, LinkRenderType, MenuRenderParams, QuoteScope,
    QuoteType, SpecialCharacter, XrefRenderParams,
};

mod parser;
pub(crate) use parser::DeferredWarning;
pub use parser::Parser;

mod path_resolver;
pub use path_resolver::PathResolver;

mod safe_mode;
pub use safe_mode::SafeMode;

mod svg_file_handler;
pub use svg_file_handler::SvgFileHandler;

pub(crate) mod preprocessor;

mod resolved_attributes;
pub(crate) use resolved_attributes::ResolvedAttributes;

mod reference_resolver;
pub use reference_resolver::{
    CatalogResolver, ReferenceResolver, ReferenceWarning, ReferenceWarningKind, ResolutionContext,
    ResolvedReference,
};

mod source_map;
pub use source_map::{SourceLine, SourceMap};
