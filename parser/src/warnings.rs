use thiserror::Error;

use crate::Span;

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

    /// An include directive specified an `encoding` attribute whose value is
    /// not UTF-8. The parser only handles UTF-8 content, so the requested
    /// encoding cannot be honored.
    #[error("include encoding is not supported (only UTF-8 is supported): {0}")]
    NonUtf8IncludeEncoding(String),
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

            WarningType::NonUtf8IncludeEncoding(encoding) => f
                .debug_tuple("WarningType::NonUtf8IncludeEncoding")
                .field(encoding)
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
            fn include_file_not_found() {
                let warning = WarningType::IncludeFileNotFound("content.adoc".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeFileNotFound(\"content.adoc\")"
                );
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
                }],
            };

            let _ = maw.unwrap_if_no_warnings();
            // There are warnings so this should panic.
        }
    }
}
