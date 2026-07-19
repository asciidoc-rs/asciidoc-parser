use thiserror::Error;

use crate::{Span, parser::SourceLine};

/// Describes a possible parse error (i.e. a "warning") and its location.
///
/// In `asciidoc-parser`, all documents are parseable, so this mechanism is used
/// to convey conditions where the parse result might be unexpected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Warning<'src> {
    /// Location where the warning was detected.
    pub source: Span<'src>,

    /// Type of warning detected.
    pub warning: WarningType,

    /// A pre-resolved originating `(file, line)` for this warning, independent
    /// of the document source map.
    ///
    /// This is `None` for the overwhelming majority of warnings: their
    /// [`source`](Self::source) span indexes the (preprocessed) document
    /// source, so the originating file and line are recovered by resolving
    /// `source.line()` through [`Document::source_map`].
    ///
    /// It is `Some` only when the warning arises from content that was expanded
    /// *privately* and never appears in the document source — an `include::`
    /// directive buried inside an owned (include-expanded) AsciiDoc table cell.
    /// No document span maps to such a directive, so its true `(file, line)` is
    /// resolved when the warning is raised (against the owning cell's own
    /// source map) and carried here directly. In that case `source` still
    /// points at the enclosing cell's directive line in the document (a
    /// best-effort anchor), but `origin` names where the failing directive
    /// actually lives.
    ///
    /// [`Document::source_map`]: crate::Document::source_map
    pub origin: Option<SourceLine>,
}

/// Type of possible parse error that was detected.
#[derive(Clone, Eq, Error, PartialEq)]
pub enum WarningType {
    #[error("An attribute value is missing its terminating quote")]
    AttributeValueMissingTerminatingQuote,

    #[error(
        "Document header wasn't terminated by a blank line (this line can't be parsed as part of a document header)"
    )]
    DocumentHeaderNotTerminated,

    #[error(
        "no inline candidate; use the inline doctype to convert a single paragraph, verbatim, or raw block"
    )]
    NoInlineDoctypeCandidate,

    #[error("An empty attribute value was detected")]
    EmptyAttributeValue,

    #[error(
        "A shorthand element attribute marker ('.', '#', or '%') was found with no subsequent text"
    )]
    EmptyShorthandItem,

    // TO DO BEFORE CHECKING IN TO MAIN: Review these error names and descriptions.
    #[error("Macro name is not a valid identifier")]
    InvalidMacroName,

    #[error("Media macro missing target")]
    MediaMacroMissingTarget,

    #[error("Macro missing attribute list")]
    MacroMissingAttributeList,

    #[error("Macro missing :: separator")]
    MacroMissingDoubleColon,

    #[error("Missing comma after quoted attribute value")]
    MissingCommaAfterQuotedAttributeValue,

    #[error("Closing marker for delimited block not found")]
    UnterminatedDelimitedBlock,

    #[error("A block title or attribute list was found without a subsequent block")]
    MissingBlockAfterTitleOrAttributeList,

    #[error("Block anchor name is empty")]
    EmptyBlockAnchorName,

    #[error("Block anchor name contains invalid name characters")]
    InvalidBlockAnchorName,

    #[error("Attribute {0:?} can not be modified by document")]
    AttributeValueIsLocked(String),

    #[error("Duplicate ID: {0:?} is already registered")]
    DuplicateId(String),

    #[error("Level 0 section headings not supported")]
    Level0SectionHeadingNotSupported,

    #[error("Section heading level skipped (expected {0}, found {1})")]
    SectionHeadingLevelSkipped(usize, usize),

    #[error("Section heading level exceeds maximum (maximum 5, found {0})")]
    SectionHeadingLevelExceedsMaximum(usize),

    #[error("Section heading level {0} is outside the supported range 1-5; clamped to {1}")]
    SectionHeadingLevelOutOfRange(i32, usize),

    #[error("leveloffset {0} places every section heading outside the supported range 1-5")]
    LeveloffsetExcludesAllHeadingLevels(i32),

    #[error("List item index: expected {0}, got {1}")]
    ListItemOutOfSequence(String, String),

    #[error("No callout found for <{0}>")]
    NoCalloutFound(usize),

    #[error("Callout list item index: expected {0}, got {1}")]
    CalloutListItemOutOfSequence(usize, usize),

    #[error("Dropping table cell because it exceeds the specified number of columns")]
    TableCellExceedsColumnCount,

    #[error("Unclosed quote in CSV data; setting cell to empty")]
    TableCsvDataHasUnclosedQuote,

    #[error("Table is missing a leading separator; recovering automatically")]
    TableMissingLeadingSeparator,

    #[error("Dropping cells from incomplete row; detected end of table")]
    TableDroppingIncompleteRowAtEndOfTable,

    #[error("skipping reference to missing attribute: {0}")]
    SkippingReferenceToMissingAttribute(String),

    #[error("invalid substitution type for stem macro: {0}")]
    InvalidSubstitutionTypeForStemMacro(String),

    #[error("invalid substitution type for passthrough macro: {0}")]
    InvalidSubstitutionTypeForPassthroughMacro(String),

    /// One or more unrecognized substitution names in a block's `subs`
    /// attribute. The names are joined with `", "`; any recognized names in
    /// the same list are still honored.
    #[error("invalid substitution type for block: {0}")]
    InvalidSubstitutionTypeForBlock(String),

    /// A footnote reference (`footnote:id[]`) names an ID that was never
    /// defined by an earlier footnote.
    #[error("invalid footnote reference: {0}")]
    InvalidFootnoteReference(String),

    /// The deprecated `footnoteref:[…]` macro was used outside compatibility
    /// mode. The footnote macro with a target should be used instead.
    #[error("found deprecated footnoteref macro: {0}; use footnote macro with target instead")]
    DeprecatedFootnorefMacro(String),

    #[error("include file not found: {0}")]
    IncludeFileNotFound(String),

    /// An include directive was not expanded because the file containing it
    /// already sits at the maximum include depth (the `max-include-depth`
    /// attribute, possibly lowered by an enclosing include directive's `depth`
    /// attribute). The field is the relative maximum in effect — the number of
    /// levels that were permitted below the file that established the limit —
    /// matching the number Asciidoctor reports.
    #[error("maximum include depth of {0} exceeded")]
    MaxIncludeDepthExceeded(usize),

    /// An include directive specified an `encoding` attribute whose value is
    /// not UTF-8. The parser only handles UTF-8 content, so the requested
    /// encoding cannot be honored.
    #[error("include encoding is not supported (only UTF-8 is supported): {0}")]
    NonUtf8IncludeEncoding(String),

    /// A conditional preprocessor directive (`ifdef`, `ifndef`, `ifeval`, or
    /// `endif`) is malformed. The first field is the specific reason (e.g.
    /// `missing target`, `target not permitted`, `missing expression`, `invalid
    /// expression`, `text not permitted`); the second is the offending
    /// directive as written.
    #[error("malformed preprocessor directive - {0}: {1}")]
    MalformedConditionalDirective(String, String),

    /// An `endif` preprocessor directive was found with no matching open
    /// conditional. The field is the offending directive as written.
    #[error("unmatched preprocessor directive: {0}")]
    UnmatchedConditionalDirective(String),

    /// An `endif` preprocessor directive names a different target than the
    /// conditional it would close. The field is the offending directive as
    /// written.
    #[error("mismatched preprocessor directive: {0}")]
    MismatchedConditionalDirective(String),

    /// A conditional preprocessor directive (`ifdef`, `ifndef`, or `ifeval`)
    /// was opened but never closed by a matching `endif`. The field is the
    /// opening directive as written.
    #[error("detected unterminated preprocessor conditional directive: {0}")]
    UnterminatedConditionalDirective(String),

    /// One or more tags named by an include directive's `tag` / `tags`
    /// attribute were never found in the include file. The field is the
    /// pre-formatted, pluralized subject — `tag '<name>'` for a single missing
    /// tag, or `tags '<name>, <name>'` (comma-joined, in the order specified)
    /// for several.
    #[error("{0} not found in include file")]
    IncludeTagNotFound(String),

    /// A tagged region in an include file was opened by a `tag::` directive but
    /// never closed. The field is the unclosed tag name.
    #[error("detected unclosed tag in include file: {0}")]
    IncludeTagUnclosed(String),

    /// An `end::` tag directive in an include file names a different tag than
    /// the region currently open. The first field is the expected (open) tag,
    /// the second is the tag actually found.
    #[error("mismatched end tag in include file (expected {0} but found {1})")]
    IncludeTagMismatchedEnd(String, String),

    /// An `end::` tag directive in an include file was found with no
    /// corresponding open region. The field is the unexpected tag name.
    #[error("unexpected end tag in include file: {0}")]
    IncludeTagUnexpectedEnd(String),

    /// An `[abstract]` block was found as a direct child of a document without
    /// a doctitle when the doctype is `book`. Asciidoctor excludes such a
    /// block's content from the converted output.
    #[error(
        "abstract block cannot be used in a document without a doctitle when doctype is book. Excluding block content."
    )]
    AbstractBlockInBookWithoutDoctitle,

    /// A cross-reference (`<<id>>` or `xref:id[…]`) named a target that the
    /// resolution pass could not resolve. The reference still renders as the
    /// unresolved fallback link (`<a href="#id">[id]</a>`). The field is the
    /// target exactly as written in the source.
    ///
    /// Asciidoctor reports this only in verbose (pedantic) mode, since a
    /// reference to an anchor that is not stored in the parse tree can be a
    /// false positive.
    #[error("possible invalid reference: {0}")]
    PossibleInvalidReference(String),
}

impl std::fmt::Debug for WarningType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarningType::AttributeValueMissingTerminatingQuote => {
                write!(f, "WarningType::AttributeValueMissingTerminatingQuote")
            }

            WarningType::DocumentHeaderNotTerminated => {
                write!(f, "WarningType::DocumentHeaderNotTerminated")
            }

            WarningType::NoInlineDoctypeCandidate => {
                write!(f, "WarningType::NoInlineDoctypeCandidate")
            }

            WarningType::EmptyAttributeValue => write!(f, "WarningType::EmptyAttributeValue"),
            WarningType::EmptyShorthandItem => write!(f, "WarningType::EmptyShorthandItem"),
            WarningType::InvalidMacroName => write!(f, "WarningType::InvalidMacroName"),

            WarningType::MediaMacroMissingTarget => {
                write!(f, "WarningType::MediaMacroMissingTarget")
            }

            WarningType::MacroMissingAttributeList => {
                write!(f, "WarningType::MacroMissingAttributeList")
            }

            WarningType::MacroMissingDoubleColon => {
                write!(f, "WarningType::MacroMissingDoubleColon")
            }

            WarningType::MissingCommaAfterQuotedAttributeValue => {
                write!(f, "WarningType::MissingCommaAfterQuotedAttributeValue")
            }

            WarningType::UnterminatedDelimitedBlock => {
                write!(f, "WarningType::UnterminatedDelimitedBlock")
            }

            WarningType::MissingBlockAfterTitleOrAttributeList => {
                write!(f, "WarningType::MissingBlockAfterTitleOrAttributeList")
            }

            WarningType::EmptyBlockAnchorName => write!(f, "WarningType::EmptyBlockAnchorName"),
            WarningType::InvalidBlockAnchorName => write!(f, "WarningType::InvalidBlockAnchorName"),

            WarningType::AttributeValueIsLocked(value) => f
                .debug_tuple("WarningType::AttributeValueIsLocked")
                .field(value)
                .finish(),

            WarningType::DuplicateId(id) => {
                f.debug_tuple("WarningType::DuplicateId").field(id).finish()
            }

            WarningType::Level0SectionHeadingNotSupported => {
                write!(f, "WarningType::Level0SectionHeadingNotSupported")
            }

            WarningType::SectionHeadingLevelSkipped(expected, found) => f
                .debug_tuple("WarningType::SectionHeadingLevelSkipped")
                .field(expected)
                .field(found)
                .finish(),

            WarningType::SectionHeadingLevelExceedsMaximum(found) => f
                .debug_tuple("WarningType::SectionHeadingLevelExceedsMaximum")
                .field(found)
                .finish(),

            WarningType::SectionHeadingLevelOutOfRange(computed, clamped) => f
                .debug_tuple("WarningType::SectionHeadingLevelOutOfRange")
                .field(computed)
                .field(clamped)
                .finish(),

            WarningType::LeveloffsetExcludesAllHeadingLevels(offset) => f
                .debug_tuple("WarningType::LeveloffsetExcludesAllHeadingLevels")
                .field(offset)
                .finish(),

            WarningType::ListItemOutOfSequence(expected, actual) => f
                .debug_tuple("WarningType::ListItemOutOfSequence")
                .field(expected)
                .field(actual)
                .finish(),

            WarningType::NoCalloutFound(number) => f
                .debug_tuple("WarningType::NoCalloutFound")
                .field(number)
                .finish(),

            WarningType::CalloutListItemOutOfSequence(expected, actual) => f
                .debug_tuple("WarningType::CalloutListItemOutOfSequence")
                .field(expected)
                .field(actual)
                .finish(),

            WarningType::TableCellExceedsColumnCount => {
                write!(f, "WarningType::TableCellExceedsColumnCount")
            }

            WarningType::TableCsvDataHasUnclosedQuote => {
                write!(f, "WarningType::TableCsvDataHasUnclosedQuote")
            }

            WarningType::TableMissingLeadingSeparator => {
                write!(f, "WarningType::TableMissingLeadingSeparator")
            }

            WarningType::TableDroppingIncompleteRowAtEndOfTable => {
                write!(f, "WarningType::TableDroppingIncompleteRowAtEndOfTable")
            }

            WarningType::SkippingReferenceToMissingAttribute(name) => f
                .debug_tuple("WarningType::SkippingReferenceToMissingAttribute")
                .field(name)
                .finish(),

            WarningType::InvalidSubstitutionTypeForStemMacro(subs) => f
                .debug_tuple("WarningType::InvalidSubstitutionTypeForStemMacro")
                .field(subs)
                .finish(),

            WarningType::InvalidSubstitutionTypeForPassthroughMacro(subs) => f
                .debug_tuple("WarningType::InvalidSubstitutionTypeForPassthroughMacro")
                .field(subs)
                .finish(),

            WarningType::InvalidSubstitutionTypeForBlock(subs) => f
                .debug_tuple("WarningType::InvalidSubstitutionTypeForBlock")
                .field(subs)
                .finish(),

            WarningType::InvalidFootnoteReference(id) => f
                .debug_tuple("WarningType::InvalidFootnoteReference")
                .field(id)
                .finish(),

            WarningType::DeprecatedFootnorefMacro(macro_text) => f
                .debug_tuple("WarningType::DeprecatedFootnorefMacro")
                .field(macro_text)
                .finish(),

            WarningType::IncludeFileNotFound(target) => f
                .debug_tuple("WarningType::IncludeFileNotFound")
                .field(target)
                .finish(),

            WarningType::MaxIncludeDepthExceeded(depth) => f
                .debug_tuple("WarningType::MaxIncludeDepthExceeded")
                .field(depth)
                .finish(),

            WarningType::NonUtf8IncludeEncoding(encoding) => f
                .debug_tuple("WarningType::NonUtf8IncludeEncoding")
                .field(encoding)
                .finish(),

            WarningType::MalformedConditionalDirective(reason, directive) => f
                .debug_tuple("WarningType::MalformedConditionalDirective")
                .field(reason)
                .field(directive)
                .finish(),

            WarningType::UnmatchedConditionalDirective(directive) => f
                .debug_tuple("WarningType::UnmatchedConditionalDirective")
                .field(directive)
                .finish(),

            WarningType::MismatchedConditionalDirective(directive) => f
                .debug_tuple("WarningType::MismatchedConditionalDirective")
                .field(directive)
                .finish(),

            WarningType::UnterminatedConditionalDirective(directive) => f
                .debug_tuple("WarningType::UnterminatedConditionalDirective")
                .field(directive)
                .finish(),

            WarningType::IncludeTagNotFound(tag) => f
                .debug_tuple("WarningType::IncludeTagNotFound")
                .field(tag)
                .finish(),

            WarningType::IncludeTagUnclosed(tag) => f
                .debug_tuple("WarningType::IncludeTagUnclosed")
                .field(tag)
                .finish(),

            WarningType::IncludeTagMismatchedEnd(expected, found) => f
                .debug_tuple("WarningType::IncludeTagMismatchedEnd")
                .field(expected)
                .field(found)
                .finish(),

            WarningType::IncludeTagUnexpectedEnd(tag) => f
                .debug_tuple("WarningType::IncludeTagUnexpectedEnd")
                .field(tag)
                .finish(),

            WarningType::AbstractBlockInBookWithoutDoctitle => {
                write!(f, "WarningType::AbstractBlockInBookWithoutDoctitle")
            }

            WarningType::PossibleInvalidReference(target) => f
                .debug_tuple("WarningType::PossibleInvalidReference")
                .field(target)
                .finish(),
        }
    }
}

/// Return type used to signal one or more possible parse error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatchAndWarnings<'src, T> {
    /// Matched item. Typically either `MatchedItem<X>` or
    /// `Option<MatchedItem<X>>`.
    pub(crate) item: T,

    /// Possible parse errors.
    pub(crate) warnings: Vec<Warning<'src>>,
}

impl<T> MatchAndWarnings<'_, T> {
    #[cfg(test)]
    #[inline(always)]
    #[track_caller]
    #[allow(clippy::panic)] // since not actually in production code
    pub(crate) fn unwrap_if_no_warnings(self) -> T {
        if self.warnings.is_empty() {
            self.item
        } else {
            panic!(
                "expected self.warnings to be empty\n\nfound warnings = {warnings:#?}\n",
                warnings = self.warnings
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod warning {
        use crate::warnings::{Warning, WarningType};

        #[test]
        fn impl_clone() {
            // Silly test to mark the #[derive(...)] line as covered.
            let w1 = Warning {
                source: crate::Span::new("abc"),
                warning: WarningType::EmptyAttributeValue,
                origin: None,
            };

            let w2 = w1.clone();
            assert_eq!(w1, w2);
        }
    }

    mod warning_type {
        mod impl_debug {
            use crate::warnings::WarningType;

            #[test]
            fn attribute_value_missing_terminating_quote() {
                let warning = WarningType::AttributeValueMissingTerminatingQuote;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AttributeValueMissingTerminatingQuote"
                );
            }

            #[test]
            fn document_header_not_terminated() {
                let warning = WarningType::DocumentHeaderNotTerminated;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::DocumentHeaderNotTerminated");
            }

            #[test]
            fn no_inline_doctype_candidate() {
                let warning = WarningType::NoInlineDoctypeCandidate;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::NoInlineDoctypeCandidate");
            }

            #[test]
            fn empty_attribute_value() {
                let warning = WarningType::EmptyAttributeValue;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::EmptyAttributeValue");
            }

            #[test]
            fn empty_shorthand_item() {
                let warning = WarningType::EmptyShorthandItem;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::EmptyShorthandItem");
            }

            #[test]
            fn invalid_macro_name() {
                let warning = WarningType::InvalidMacroName;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::InvalidMacroName");
            }

            #[test]
            fn media_macro_missing_target() {
                let warning = WarningType::MediaMacroMissingTarget;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MediaMacroMissingTarget");
            }

            #[test]
            fn macro_missing_attribute_list() {
                let warning = WarningType::MacroMissingAttributeList;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MacroMissingAttributeList");
            }

            #[test]
            fn macro_missing_double_colon() {
                let warning = WarningType::MacroMissingDoubleColon;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MacroMissingDoubleColon");
            }

            #[test]
            fn missing_comma_after_quoted_attribute_value() {
                let warning = WarningType::MissingCommaAfterQuotedAttributeValue;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::MissingCommaAfterQuotedAttributeValue"
                );
            }

            #[test]
            fn unterminated_delimited_block() {
                let warning = WarningType::UnterminatedDelimitedBlock;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::UnterminatedDelimitedBlock");
            }

            #[test]
            fn missing_block_after_title_or_attribute_list() {
                let warning = WarningType::MissingBlockAfterTitleOrAttributeList;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::MissingBlockAfterTitleOrAttributeList"
                );
            }

            #[test]
            fn empty_block_anchor_name() {
                let warning = WarningType::EmptyBlockAnchorName;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::EmptyBlockAnchorName");
            }

            #[test]
            fn invalid_block_anchor_name() {
                let warning = WarningType::InvalidBlockAnchorName;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::InvalidBlockAnchorName");
            }

            #[test]
            fn attribute_value_is_locked_simple_string() {
                let warning = WarningType::AttributeValueIsLocked("test-attribute".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AttributeValueIsLocked(\"test-attribute\")"
                );
            }

            #[test]
            fn attribute_value_is_locked_empty_string() {
                let warning = WarningType::AttributeValueIsLocked("".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::AttributeValueIsLocked(\"\")");
            }

            #[test]
            fn attribute_value_is_locked_string_with_special_chars() {
                let warning =
                    WarningType::AttributeValueIsLocked("attr-with-special!@#$%^&*()".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AttributeValueIsLocked(\"attr-with-special!@#$%^&*()\")"
                );
            }

            #[test]
            fn attribute_value_is_locked_string_with_quotes() {
                let warning = WarningType::AttributeValueIsLocked("attr\"with'quotes".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AttributeValueIsLocked(\"attr\\\"with'quotes\")"
                );
            }

            #[test]
            fn attribute_value_is_locked_string_with_newlines() {
                let warning =
                    WarningType::AttributeValueIsLocked("attr\nwith\nnewlines".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AttributeValueIsLocked(\"attr\\nwith\\nnewlines\")"
                );
            }

            #[test]
            fn duplicate_id() {
                let warning = WarningType::DuplicateId("foo".to_owned());
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::DuplicateId(\"foo\")");
            }

            #[test]
            fn level0_section_heading_not_supported() {
                let warning = WarningType::Level0SectionHeadingNotSupported;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::Level0SectionHeadingNotSupported"
                );
            }

            #[test]
            fn section_heading_level_skipped() {
                let warning = WarningType::SectionHeadingLevelSkipped(2, 4);
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::SectionHeadingLevelSkipped(2, 4)"
                );
            }

            #[test]
            fn section_heading_level_exceeds_maximum() {
                let warning = WarningType::SectionHeadingLevelExceedsMaximum(6);
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::SectionHeadingLevelExceedsMaximum(6)"
                );
            }

            #[test]
            fn section_heading_level_out_of_range() {
                let warning = WarningType::SectionHeadingLevelOutOfRange(-3, 1);
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::SectionHeadingLevelOutOfRange(-3, 1)"
                );
            }

            #[test]
            fn leveloffset_excludes_all_heading_levels() {
                let warning = WarningType::LeveloffsetExcludesAllHeadingLevels(2147483647);
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::LeveloffsetExcludesAllHeadingLevels(2147483647)"
                );
            }

            #[test]
            fn list_item_out_of_sequence() {
                let warning = WarningType::ListItemOutOfSequence("y".to_string(), "z".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::ListItemOutOfSequence(\"y\", \"z\")"
                );
            }

            #[test]
            fn no_callout_found() {
                let warning = WarningType::NoCalloutFound(2);
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::NoCalloutFound(2)");
            }

            #[test]
            fn callout_list_item_out_of_sequence() {
                let warning = WarningType::CalloutListItemOutOfSequence(2, 3);
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::CalloutListItemOutOfSequence(2, 3)"
                );
            }

            #[test]
            fn table_cell_exceeds_column_count() {
                let warning = WarningType::TableCellExceedsColumnCount;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::TableCellExceedsColumnCount");
            }

            #[test]
            fn table_csv_data_has_unclosed_quote() {
                let warning = WarningType::TableCsvDataHasUnclosedQuote;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::TableCsvDataHasUnclosedQuote");
            }

            #[test]
            fn table_missing_leading_separator() {
                let warning = WarningType::TableMissingLeadingSeparator;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::TableMissingLeadingSeparator");
            }

            #[test]
            fn table_dropping_incomplete_row_at_end_of_table() {
                let warning = WarningType::TableDroppingIncompleteRowAtEndOfTable;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::TableDroppingIncompleteRowAtEndOfTable"
                );
            }

            #[test]
            fn skipping_reference_to_missing_attribute() {
                let warning = WarningType::SkippingReferenceToMissingAttribute("name".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::SkippingReferenceToMissingAttribute(\"name\")"
                );
            }

            #[test]
            fn invalid_substitution_type_for_stem_macro() {
                let warning = WarningType::InvalidSubstitutionTypeForStemMacro("bogus".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::InvalidSubstitutionTypeForStemMacro(\"bogus\")"
                );
            }

            #[test]
            fn invalid_substitution_type_for_passthrough_macro() {
                let warning =
                    WarningType::InvalidSubstitutionTypeForPassthroughMacro("bogus".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::InvalidSubstitutionTypeForPassthroughMacro(\"bogus\")"
                );
            }

            #[test]
            fn invalid_substitution_type_for_block() {
                let warning = WarningType::InvalidSubstitutionTypeForBlock("bogus".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::InvalidSubstitutionTypeForBlock(\"bogus\")"
                );
            }

            #[test]
            fn invalid_footnote_reference() {
                let warning = WarningType::InvalidFootnoteReference("fn1".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::InvalidFootnoteReference(\"fn1\")"
                );
            }

            #[test]
            fn deprecated_footnoref_macro() {
                let warning =
                    WarningType::DeprecatedFootnorefMacro("footnoteref:[fn1]".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::DeprecatedFootnorefMacro(\"footnoteref:[fn1]\")"
                );
            }

            #[test]
            fn include_file_not_found() {
                let warning = WarningType::IncludeFileNotFound("content.adoc".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeFileNotFound(\"content.adoc\")"
                );
            }

            #[test]
            fn max_include_depth_exceeded() {
                let warning = WarningType::MaxIncludeDepthExceeded(64);
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MaxIncludeDepthExceeded(64)");
            }

            #[test]
            fn non_utf8_include_encoding() {
                let warning = WarningType::NonUtf8IncludeEncoding("iso-8859-1".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::NonUtf8IncludeEncoding(\"iso-8859-1\")"
                );
            }

            #[test]
            fn malformed_conditional_directive() {
                let warning = WarningType::MalformedConditionalDirective(
                    "missing target".to_string(),
                    "ifdef::[]".to_string(),
                );
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::MalformedConditionalDirective(\"missing target\", \"ifdef::[]\")"
                );
            }

            #[test]
            fn unmatched_conditional_directive() {
                let warning =
                    WarningType::UnmatchedConditionalDirective("endif::on-quest[]".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::UnmatchedConditionalDirective(\"endif::on-quest[]\")"
                );
            }

            #[test]
            fn mismatched_conditional_directive() {
                let warning =
                    WarningType::MismatchedConditionalDirective("endif::on-journey[]".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::MismatchedConditionalDirective(\"endif::on-journey[]\")"
                );
            }

            #[test]
            fn unterminated_conditional_directive() {
                let warning =
                    WarningType::UnterminatedConditionalDirective("ifdef::on-quest[]".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::UnterminatedConditionalDirective(\"ifdef::on-quest[]\")"
                );
            }

            #[test]
            fn include_tag_not_found() {
                let warning = WarningType::IncludeTagNotFound("tag 'no-such-tag'".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeTagNotFound(\"tag 'no-such-tag'\")"
                );
            }

            #[test]
            fn include_tag_unclosed() {
                let warning = WarningType::IncludeTagUnclosed("'a'".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::IncludeTagUnclosed(\"'a'\")");
            }

            #[test]
            fn include_tag_mismatched_end() {
                let warning =
                    WarningType::IncludeTagMismatchedEnd("'b'".to_string(), "'a'".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeTagMismatchedEnd(\"'b'\", \"'a'\")"
                );
            }

            #[test]
            fn include_tag_unexpected_end() {
                let warning = WarningType::IncludeTagUnexpectedEnd("'a'".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeTagUnexpectedEnd(\"'a'\")"
                );
            }

            #[test]
            fn abstract_block_in_book_without_doctitle() {
                let warning = WarningType::AbstractBlockInBookWithoutDoctitle;
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::AbstractBlockInBookWithoutDoctitle"
                );
            }

            #[test]
            fn possible_invalid_reference() {
                let warning = WarningType::PossibleInvalidReference("foobaz".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::PossibleInvalidReference(\"foobaz\")"
                );
            }
        }
    }

    mod match_and_warnings {
        use crate::warnings::{MatchAndWarnings, Warning, WarningType};

        #[test]
        fn impl_clone() {
            // Silly test to mark the #[derive(...)] line as covered.
            let maw1 = MatchAndWarnings {
                item: "xyz",
                warnings: vec![Warning {
                    source: crate::Span::new("abc"),
                    warning: WarningType::EmptyAttributeValue,
                    origin: None,
                }],
            };

            let maw2 = maw1.clone();
            assert_eq!(maw1, maw2);
        }

        #[test]
        fn unwrap_if_no_warnings() {
            let maw = MatchAndWarnings {
                item: "xyz",
                warnings: vec![],
            };

            let item = maw.unwrap_if_no_warnings();
            assert_eq!(item, "xyz");
        }

        #[test]
        #[should_panic]
        fn unwrap_if_no_warnings_panic() {
            let maw = MatchAndWarnings {
                item: "xyz",
                warnings: vec![Warning {
                    source: crate::Span::new("abc"),
                    warning: WarningType::EmptyAttributeValue,
                    origin: None,
                }],
            };

            let _ = maw.unwrap_if_no_warnings();
            // There are warnings so this should panic.
        }
    }
}
