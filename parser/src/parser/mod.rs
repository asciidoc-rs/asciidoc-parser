//! The [`Parser`] struct and its related structs allow a caller to configure
//! how AsciiDoc parsing occurs and then to initiate the parsing process.

mod attribute_value;
pub(crate) use attribute_value::AttributeValue;
pub use attribute_value::{AllowableValue, ModificationContext};

mod built_in_attrs;
pub(crate) use built_in_attrs::{built_in_attr, built_in_attrs_iter};

mod docinfo_file_handler;
pub use docinfo_file_handler::DocinfoFileHandler;

mod image_file_handler;
pub use image_file_handler::ImageFileHandler;

mod include_file_handler;
pub use include_file_handler::{IncludeContent, IncludeFileHandler};

mod inline_substitution_renderer;
pub use inline_substitution_renderer::{
    CalloutGuard, CalloutRenderParams, CharacterReplacementType, FootnoteRenderParams,
    HtmlSubstitutionRenderer, IconRenderParams, ImageRenderParams, IndexTermRenderParams,
    InlineSubstitutionRenderer, LinkRenderParams, MenuRenderParams, QuoteScope, QuoteType,
    SpecialCharacter, XrefRenderParams,
};
pub(crate) use inline_substitution_renderer::{
    has_dangerous_scheme, has_dangerous_self_href, is_uri_ish,
};

mod parser;
pub(crate) use parser::DeferredWarning;
pub use parser::Parser;

mod path_resolver;
pub use path_resolver::{DefaultPathResolver, PathResolver};

mod safe_mode;
pub use safe_mode::SafeMode;

mod svg_file_handler;
pub use svg_file_handler::SvgFileHandler;

pub(crate) mod preprocessor;

mod resolved_attributes;
pub(crate) use resolved_attributes::{ResolvedAttributes, attribute_lookup_name};

mod reference_resolver;
pub(crate) use reference_resolver::ReferenceWarnings;
pub use reference_resolver::{
    CatalogResolver, DerivedReference, ReferenceResolver, ReferenceWarning, ReferenceWarningKind,
    ResolutionContext, ResolvedReference, XrefSignifier, XrefStyle,
};

mod reference_time;
pub use reference_time::ReferenceTime;
pub(crate) use reference_time::{DatetimeContext, DatetimeInputs, is_datetime_attribute};

mod source_map;
pub use source_map::{Fidelity, Origin, SourceLine, SourceMap, Transform};
