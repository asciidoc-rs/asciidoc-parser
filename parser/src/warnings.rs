//! Describes conditions where a parse result might be unexpected.
//!
//! Every UTF-8 string is a valid AsciiDoc document, so parsing never fails.
//! Anything ambiguous or likely unintended is reported as a [`Warning`]
//! instead, and a caller is advised to review the warnings a parse produced
//! (see [`Document::warnings`](crate::Document::warnings)).

use thiserror::Error;

use crate::{Span, parser::SourceLine};

/// The severity of a [`Warning`].
///
/// Every diagnostic this parser records is surfaced through
/// [`Document::warnings`](crate::Document::warnings) regardless of severity; a
/// host filters on this value to decide which diagnostics to act on. The
/// variants are ordered from least to most severe, so a host can select "at or
/// above" a threshold with an ordinary comparison – for example, keeping only
/// entries where `warning.severity >= WarningSeverity::Warning` suppresses the
/// low-severity [`Debug`](Self::Debug) diagnostics.
///
/// A warning's severity is an intrinsic property of its
/// [`WarningType`](WarningType); it is assigned when the warning is constructed
/// and never varies from one occurrence of a given type to another.
///
/// This enum is `non_exhaustive`: further severities may be recognized as the
/// parser grows, so a host matching on it needs a catch-all arm.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash)]
#[non_exhaustive]
pub enum WarningSeverity {
    /// A low-severity diagnostic that a host is expected to suppress by
    /// default. It reports something a host may wish to observe (for example,
    /// via tooling or a verbose mode) but which does not, on its own, suggest
    /// the parse result is wrong. Asciidoctor logs the equivalent messages
    /// below its default `WARN` threshold.
    Debug,

    /// A condition where the parse result might be unexpected. This is the
    /// severity of the overwhelming majority of this parser's diagnostics and
    /// the level a host should surface by default.
    Warning,
}

impl std::fmt::Debug for WarningSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarningSeverity::Debug => write!(f, "WarningSeverity::Debug"),
            WarningSeverity::Warning => write!(f, "WarningSeverity::Warning"),
        }
    }
}

/// Describes a possible parse error (i.e. a "warning") and its location.
///
/// In `asciidoc-parser`, all documents are parseable, so this mechanism is used
/// to convey conditions where the parse result might be unexpected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Warning<'src> {
    /// Location where the warning was detected.
    pub source: Span<'src>,

    /// Type of warning detected.
    pub warning: WarningType,

    /// Severity of this warning.
    ///
    /// This is derived from [`warning`](Self::warning) – each
    /// [`WarningType`](WarningType) has a fixed severity – so a host can filter
    /// the [`Document::warnings`](crate::Document::warnings) stream by
    /// importance without matching on every individual type. Most diagnostics
    /// are [`WarningSeverity::Warning`]; a handful (such as an unknown block
    /// style) are [`WarningSeverity::Debug`], which a host suppresses by
    /// default.
    pub severity: WarningSeverity,

    /// A pre-resolved originating `(file, line)` for this warning, independent
    /// of the document source map.
    ///
    /// This is `None` for the overwhelming majority of warnings: their
    /// [`source`](Self::source) span indexes the (preprocessed) document
    /// source, so the originating file and line are recovered by resolving
    /// `source.line()` through [`Document::source_map`].
    ///
    /// It is `Some` only when the warning arises from content that was expanded
    /// *privately* and never appears in the document source – an `include::`
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

impl<'src> Warning<'src> {
    /// Build a warning anchored at `source`, taking its severity from
    /// `warning`'s [`WarningType::severity`] and leaving
    /// [`origin`](Self::origin) unset. This is the constructor used for the
    /// overwhelming majority of warnings, whose location is recovered from the
    /// document source map.
    pub(crate) fn new(source: Span<'src>, warning: WarningType) -> Self {
        let severity = warning.severity();

        Self {
            source,
            warning,
            severity,
            origin: None,
        }
    }

    /// Build a warning anchored at `source` and carrying a pre-resolved
    /// [`origin`](Self::origin), taking its severity from `warning`'s
    /// [`WarningType::severity`]. This is used for the rare warnings that arise
    /// from privately-expanded content with no document span of its own (see
    /// [`origin`](Self::origin)).
    pub(crate) fn with_origin(
        source: Span<'src>,
        warning: WarningType,
        origin: Option<SourceLine>,
    ) -> Self {
        let severity = warning.severity();

        Self {
            source,
            warning,
            severity,
            origin,
        }
    }
}

/// Type of possible parse error that was detected.
///
/// This enum is `non_exhaustive`: new conditions are recognized as the parser
/// grows, so a host matching on it needs a catch-all arm.
#[derive(Clone, Eq, Error, Hash, PartialEq)]
#[non_exhaustive]
pub enum WarningType {
    /// A quoted attribute value ran to the end of its line (or the end of the
    /// attribute list) without a matching closing quote.
    #[error("an attribute value is missing its terminating quote")]
    AttributeValueMissingTerminatingQuote,

    /// A document header was not followed by a blank line, so the line that
    /// follows it can not be parsed as part of the header.
    #[error(
        "document header wasn't terminated by a blank line (this line can't be parsed as part of a document header)"
    )]
    DocumentHeaderNotTerminated,

    /// The `inline` doctype was requested for a document that holds no single
    /// paragraph, verbatim, or raw block to convert.
    #[error(
        "no inline candidate; use the inline doctype to convert a single paragraph, verbatim, or raw block"
    )]
    NoInlineDoctypeCandidate,

    /// An element attribute was written with a name and `=` but no value.
    #[error("an empty attribute value was detected")]
    EmptyAttributeValue,

    /// A shorthand element attribute marker (`.` for a role, `#` for an ID, or
    /// `%` for an option) was found with no name after it.
    #[error(
        "a shorthand element attribute marker ('.', '#', or '%') was found with no subsequent text"
    )]
    EmptyShorthandName,

    /// The name in a block or inline macro is not a valid identifier.
    #[error("macro name is not a valid identifier")]
    InvalidMacroName,

    /// A media macro (`image::`, `video::`, or `audio::`) was written without
    /// the target that names the media to embed.
    #[error("media macro missing target")]
    MediaMacroMissingTarget,

    /// A macro was written without the `[…]` attribute list that terminates it.
    #[error("macro missing attribute list")]
    MacroMissingAttributeList,

    /// A block macro was written without the `::` that separates its name from
    /// its target.
    #[error("macro missing :: separator")]
    MacroMissingSeparator,

    /// A quoted attribute value in an attribute list was followed by something
    /// other than the comma that separates it from the next attribute.
    #[error("missing comma after quoted attribute value")]
    MissingCommaAfterQuotedAttributeValue,

    /// A delimited block was opened but the matching closing delimiter was
    /// never found, so the block runs to the end of the document.
    #[error("closing marker for delimited block not found")]
    UnterminatedDelimitedBlock,

    /// A block title (`.Title`) or attribute list (`[…]`) was found at the end
    /// of the document or immediately before a blank line, with no block for it
    /// to describe.
    #[error("a block title or attribute list was found without a subsequent block")]
    MissingBlockAfterTitleOrAttributeList,

    /// A block anchor (`[[…]]`) was written with no name between its brackets.
    #[error("block anchor name is empty")]
    EmptyBlockAnchorName,

    /// A block anchor (`[[…]]`) names an ID containing characters that are not
    /// permitted in a name.
    #[error("block anchor name contains invalid name characters")]
    InvalidBlockAnchorName,

    /// The document tried to set an attribute that the API caller locked when
    /// it configured the parser. The field is the attribute name.
    #[error("attribute {0:?} can not be modified by document")]
    AttributeValueIsLocked(String),

    /// An ID was assigned to an element when an earlier element had already
    /// registered it. The field is the duplicated ID.
    #[error("duplicate ID: {0:?} is already registered")]
    DuplicateId(String),

    /// A level-0 section heading (`= Title`) was found somewhere other than the
    /// document header, where this crate does not support it.
    #[error("level 0 section headings not supported")]
    Level0SectionHeadingNotSupported,

    /// A section heading skipped one or more levels below its parent. The
    /// fields are the expected level and the level actually found.
    #[error("section heading level skipped (expected {0}, found {1})")]
    SectionHeadingLevelSkipped(usize, usize),

    /// A section heading nests deeper than the deepest supported level. The
    /// field is the level found.
    #[error("section heading level exceeds maximum (maximum 5, found {0})")]
    SectionHeadingLevelExceedsMaximum(usize),

    /// A `leveloffset` shifted a section heading outside the supported range,
    /// so its level was clamped. The fields are the offset level and the level
    /// it was clamped to.
    #[error("section heading level {0} is outside the supported range 1-5; clamped to {1}")]
    SectionHeadingLevelOutOfRange(i32, usize),

    /// A `leveloffset` is so large (or so negative) that no authored heading
    /// level could land inside the supported range. The field is the offset.
    #[error("leveloffset {0} places every section heading outside the supported range 1-5")]
    LeveloffsetExcludesAllHeadingLevels(i32),

    /// A special section that does not support nested sections (a `glossary`,
    /// `bibliography`, `colophon`, `dedication`, or `index` section) contains a
    /// subsection. The field is the section's style name. Mirrors Asciidoctor,
    /// which permits subsections only within the `appendix`, `preface`, and
    /// `abstract` special sections.
    #[error("{0} sections do not support nested sections")]
    SpecialSectionCannotHaveNestedSections(String),

    /// An explicitly-numbered list item does not continue the sequence its list
    /// established. The fields are the expected and actual indexes.
    #[error("list item index: expected {0}, got {1}")]
    ListItemOutOfSequence(String, String),

    /// A callout list item has no matching callout marker in the verbatim block
    /// it annotates. The field is the callout number.
    #[error("no callout found for <{0}>")]
    NoCalloutFound(usize),

    /// A callout list item does not continue the sequence its list established.
    /// The fields are the expected and actual indexes.
    #[error("callout list item index: expected {0}, got {1}")]
    CalloutListItemOutOfSequence(usize, usize),

    /// A table row holds more cells than the table's column count allows; the
    /// surplus cell is dropped.
    #[error("dropping table cell because it exceeds the specified number of columns")]
    TableCellExceedsColumnCount,

    /// A quoted field in a CSV-format table was never closed; the cell is set
    /// to empty.
    #[error("unclosed quote in CSV data; setting cell to empty")]
    TableCsvDataHasUnclosedQuote,

    /// A table row does not begin with the cell separator its table uses;
    /// parsing recovers by assuming one.
    #[error("table is missing a leading separator; recovering automatically")]
    TableMissingLeadingSeparator,

    /// A table ended part-way through a row; the cells of that partial row are
    /// dropped.
    #[error("dropping cells from incomplete row; detected end of table")]
    TableIncompleteRowAtEndOfTable,

    /// An attribute reference (`{name}`) names an attribute that is not set,
    /// under `attribute-missing=warn`. The field is the attribute name.
    #[error("skipping reference to missing attribute: {0}")]
    SkippingReferenceToMissingAttribute(String),

    /// A `stem:` macro named a substitution type that is not recognized. The
    /// field is the unrecognized name.
    #[error("invalid substitution type for stem macro: {0}")]
    InvalidSubstitutionTypeForStemMacro(String),

    /// A passthrough macro (`pass:`) named a substitution type that is not
    /// recognized. The field is the unrecognized name.
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
    DeprecatedFootnoterefMacro(String),

    /// An `include::` directive named a file that the configured include file
    /// handler could not resolve. The field is the target as written.
    #[error("include file not found: {0}")]
    IncludeFileNotFound(String),

    /// An `include::` directive named a file that exists but the configured
    /// include file handler could not read (for example a permission or other
    /// IO error). The field is the target as written. Asciidoctor distinguishes
    /// this from [`IncludeFileNotFound`](Self::IncludeFileNotFound), logging
    /// `include file not readable` rather than `include file not found`.
    #[error("include file not readable: {0}")]
    IncludeFileNotReadable(String),

    /// An `include::` directive named a file that the configured include file
    /// handler found and read but which is not valid UTF-8, and which the
    /// handler could not transcode (no `encoding` attribute, or one it does not
    /// support). The field is the target as written. Asciidoctor treats this as
    /// a fatal error (`invalid byte sequence in UTF-8`) that aborts the
    /// conversion; this crate favors recoverable warnings, so it drops the
    /// include and records this warning instead. This is distinct from
    /// [`NonUtf8IncludeEncoding`](Self::NonUtf8IncludeEncoding), which reports
    /// an unsupported `encoding` request for content the handler *did* return.
    #[error("include file not decodable (invalid byte sequence in UTF-8): {0}")]
    IncludeFileNotDecodable(String),

    /// An include directive's target referenced a missing attribute while
    /// `attribute-missing` was set to `warn`, so the directive was dropped
    /// without being resolved. (Under `drop-line` the directive line is
    /// removed silently instead.) The field is the directive as written.
    #[error("include dropped due to missing attribute: {0}")]
    IncludeDroppedDueToMissingAttribute(String),

    /// An include directive was not expanded because the file containing it
    /// already sits at the maximum include depth (the `max-include-depth`
    /// attribute, possibly lowered by an enclosing include directive's `depth`
    /// attribute). The field is the relative maximum in effect – the number of
    /// levels that were permitted below the file that established the limit –
    /// matching the number Asciidoctor reports.
    #[error("maximum include depth of {0} exceeded")]
    MaxIncludeDepthExceeded(usize),

    /// Block parsing reached the maximum nesting depth (the `max-block-nesting`
    /// attribute, default 32, API-only) before the innermost content was
    /// parsed, so the over-nested content was truncated rather than descended
    /// into. This bounds native recursion – a delimited block's body, a section
    /// body, a table cell, or a nested list each parse on a fresh call stack –
    /// so a crafted document cannot overflow the stack and abort the process.
    /// The field is the limit in effect.
    #[error("maximum block nesting depth of {0} exceeded")]
    MaxBlockNestingExceeded(usize),

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
    /// pre-formatted, pluralized subject – `tag '<name>'` for a single missing
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

    /// An explicit `link:` macro named a target whose URI scheme can execute
    /// script (`javascript:`, `data:`, or `vbscript:`). The macro is not turned
    /// into a link; it is left as literal source text instead. The field is the
    /// target exactly as written. This is a security measure with no
    /// counterpart in Ruby Asciidoctor.
    #[error("rejected link with potentially unsafe scheme (rendered as text): {0}")]
    UnsafeLinkSchemeRejected(String),

    /// A block declared a style (the first positional attribute, e.g. `[foo]`)
    /// that this parser does not recognize for the block's context, and which
    /// no built-in masquerade accepts. The style is retained on the block but
    /// otherwise ignored: the block keeps the default context implied by its
    /// syntax (for example, `[foo]` over a `--` delimiter stays an open block).
    ///
    /// The first field is the block's context (for example `open` or
    /// `paragraph`); the second is the unrecognized style as the author wrote
    /// it. This is a [`WarningSeverity::Debug`] diagnostic – Asciidoctor logs
    /// the equivalent message (`unknown style for <context> block: <style>`)
    /// only below its default `WARN` threshold – so a host suppresses it by
    /// default.
    #[error("unknown style for {0} block: {1}")]
    UnknownBlockStyle(String, String),
}

impl WarningType {
    /// The intrinsic [`WarningSeverity`] of this warning type.
    ///
    /// Almost every type is [`WarningSeverity::Warning`]; the exceptions are
    /// low-severity diagnostics (such as [`UnknownBlockStyle`]) that a host
    /// suppresses by default. [`Warning::new`] uses this to stamp each
    /// constructed [`Warning`] with its severity.
    ///
    /// [`UnknownBlockStyle`]: Self::UnknownBlockStyle
    pub(crate) fn severity(&self) -> WarningSeverity {
        match self {
            WarningType::UnknownBlockStyle(..) => WarningSeverity::Debug,
            _ => WarningSeverity::Warning,
        }
    }
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
            WarningType::EmptyShorthandName => write!(f, "WarningType::EmptyShorthandName"),
            WarningType::InvalidMacroName => write!(f, "WarningType::InvalidMacroName"),

            WarningType::MediaMacroMissingTarget => {
                write!(f, "WarningType::MediaMacroMissingTarget")
            }

            WarningType::MacroMissingAttributeList => {
                write!(f, "WarningType::MacroMissingAttributeList")
            }

            WarningType::MacroMissingSeparator => {
                write!(f, "WarningType::MacroMissingSeparator")
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

            WarningType::SpecialSectionCannotHaveNestedSections(style) => f
                .debug_tuple("WarningType::SpecialSectionCannotHaveNestedSections")
                .field(style)
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

            WarningType::TableIncompleteRowAtEndOfTable => {
                write!(f, "WarningType::TableIncompleteRowAtEndOfTable")
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

            WarningType::DeprecatedFootnoterefMacro(macro_text) => f
                .debug_tuple("WarningType::DeprecatedFootnoterefMacro")
                .field(macro_text)
                .finish(),

            WarningType::IncludeFileNotFound(target) => f
                .debug_tuple("WarningType::IncludeFileNotFound")
                .field(target)
                .finish(),

            WarningType::IncludeFileNotReadable(target) => f
                .debug_tuple("WarningType::IncludeFileNotReadable")
                .field(target)
                .finish(),

            WarningType::IncludeFileNotDecodable(target) => f
                .debug_tuple("WarningType::IncludeFileNotDecodable")
                .field(target)
                .finish(),

            WarningType::IncludeDroppedDueToMissingAttribute(directive) => f
                .debug_tuple("WarningType::IncludeDroppedDueToMissingAttribute")
                .field(directive)
                .finish(),

            WarningType::MaxIncludeDepthExceeded(depth) => f
                .debug_tuple("WarningType::MaxIncludeDepthExceeded")
                .field(depth)
                .finish(),

            WarningType::MaxBlockNestingExceeded(depth) => f
                .debug_tuple("WarningType::MaxBlockNestingExceeded")
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

            WarningType::UnsafeLinkSchemeRejected(target) => f
                .debug_tuple("WarningType::UnsafeLinkSchemeRejected")
                .field(target)
                .finish(),

            WarningType::UnknownBlockStyle(context, style) => f
                .debug_tuple("WarningType::UnknownBlockStyle")
                .field(context)
                .field(style)
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
            let w1 = Warning::new(crate::Span::new("abc"), WarningType::EmptyAttributeValue);

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
            fn empty_shorthand_name() {
                let warning = WarningType::EmptyShorthandName;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::EmptyShorthandName");
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
            fn macro_missing_separator() {
                let warning = WarningType::MacroMissingSeparator;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MacroMissingSeparator");
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
            fn special_section_cannot_have_nested_sections() {
                let warning =
                    WarningType::SpecialSectionCannotHaveNestedSections("glossary".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::SpecialSectionCannotHaveNestedSections(\"glossary\")"
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
            fn table_incomplete_row_at_end_of_table() {
                let warning = WarningType::TableIncompleteRowAtEndOfTable;
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::TableIncompleteRowAtEndOfTable");
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
            fn deprecated_footnoteref_macro() {
                let warning =
                    WarningType::DeprecatedFootnoterefMacro("footnoteref:[fn1]".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::DeprecatedFootnoterefMacro(\"footnoteref:[fn1]\")"
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
            fn include_file_not_readable() {
                let warning = WarningType::IncludeFileNotReadable("content.adoc".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeFileNotReadable(\"content.adoc\")"
                );
            }

            #[test]
            fn include_file_not_decodable() {
                let warning = WarningType::IncludeFileNotDecodable("content.adoc".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::IncludeFileNotDecodable(\"content.adoc\")"
                );
            }

            #[test]
            fn include_dropped_due_to_missing_attribute() {
                let warning = WarningType::IncludeDroppedDueToMissingAttribute(
                    "include::{foodir}/include-file.adoc[]".to_string(),
                );

                let debug_output = format!("{:?}", warning);

                assert_eq!(
                    debug_output,
                    "WarningType::IncludeDroppedDueToMissingAttribute(\"include::{foodir}/include-file.adoc[]\")"
                );
            }

            #[test]
            fn max_include_depth_exceeded() {
                let warning = WarningType::MaxIncludeDepthExceeded(64);
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MaxIncludeDepthExceeded(64)");
            }

            #[test]
            fn max_block_nesting_exceeded() {
                let warning = WarningType::MaxBlockNestingExceeded(64);
                let debug_output = format!("{:?}", warning);
                assert_eq!(debug_output, "WarningType::MaxBlockNestingExceeded(64)");
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

            #[test]
            fn unsafe_link_scheme_rejected() {
                let warning =
                    WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::UnsafeLinkSchemeRejected(\"javascript:alert(1)\")"
                );
            }

            #[test]
            fn unknown_block_style() {
                let warning = WarningType::UnknownBlockStyle("open".to_string(), "foo".to_string());
                let debug_output = format!("{:?}", warning);
                assert_eq!(
                    debug_output,
                    "WarningType::UnknownBlockStyle(\"open\", \"foo\")"
                );
            }
        }

        mod severity {
            use crate::warnings::{WarningSeverity, WarningType};

            #[test]
            fn unknown_block_style_is_debug() {
                let warning = WarningType::UnknownBlockStyle("open".to_string(), "foo".to_string());
                assert_eq!(warning.severity(), WarningSeverity::Debug);
            }

            #[test]
            fn most_types_are_warning() {
                assert_eq!(
                    WarningType::EmptyAttributeValue.severity(),
                    WarningSeverity::Warning
                );
            }

            #[test]
            fn debug_is_less_severe_than_warning() {
                assert!(WarningSeverity::Debug < WarningSeverity::Warning);
            }

            #[test]
            fn impl_debug() {
                assert_eq!(
                    format!("{:?}", WarningSeverity::Debug),
                    "WarningSeverity::Debug"
                );
                assert_eq!(
                    format!("{:?}", WarningSeverity::Warning),
                    "WarningSeverity::Warning"
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
                warnings: vec![Warning::new(
                    crate::Span::new("abc"),
                    WarningType::EmptyAttributeValue,
                )],
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
                warnings: vec![Warning::new(
                    crate::Span::new("abc"),
                    WarningType::EmptyAttributeValue,
                )],
            };

            let _ = maw.unwrap_if_no_warnings();

            // There are warnings so this should panic.
        }
    }
}
