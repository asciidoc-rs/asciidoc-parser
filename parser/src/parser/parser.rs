use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    Document, HasSpan,
    blocks::{SectionNumber, SectionType},
    document::{Attribute, Catalog, InterpretedValue, RefType},
    parser::{
        AllowableValue, AttributeValue, HtmlSubstitutionRenderer, IncludeFileHandler,
        InlineSubstitutionRenderer, ModificationContext, PathResolver,
        built_in_attrs::{built_in_attrs, built_in_default_values},
        preprocessor::preprocess,
    },
    warnings::{Warning, WarningType},
};

/// The [`Parser`] struct and its related structs allow a caller to configure
/// how AsciiDoc parsing occurs and then to initiate the parsing process.
#[derive(Clone, Debug)]
pub struct Parser {
    /// Attribute values at current state of parsing.
    pub(crate) attribute_values: HashMap<String, AttributeValue>,

    /// Default values for attributes if "set."
    default_attribute_values: HashMap<String, String>,

    /// Specifies how the basic raw text of a simple block will be converted to
    /// the format which will ultimately be presented in the final output.
    ///
    /// Typically this is an [`HtmlSubstitutionRenderer`] but clients may
    /// provide alternative implementations.
    pub(crate) renderer: Rc<dyn InlineSubstitutionRenderer>,

    /// Specifies the name of the primary file to be parsed.
    pub(crate) primary_file_name: Option<String>,

    /// Specifies how to generate clean and secure paths relative to the parsing
    /// context.
    pub path_resolver: PathResolver,

    /// Handler for resolving include:: directives.
    pub(crate) include_file_handler: Option<Rc<dyn IncludeFileHandler>>,

    /// Document catalog for tracking referenceable elements during parsing.
    /// This is created during parsing and transferred to the Document when
    /// complete.
    ///
    /// Wrapped in a [`RefCell`] so that anchors and references discovered deep
    /// inside inline substitution (where only a shared `&Parser` is available,
    /// e.g. within a regex [`Replacer`](regex::Replacer)) can still be
    /// registered.
    catalog: RefCell<Catalog>,

    /// Most recently-assigned section number.
    pub(crate) last_section_number: SectionNumber,

    /// Most recently-assigned appendix section number.
    pub(crate) last_appendix_section_number: SectionNumber,

    /// Saved copy of sectnumlevels at end of document header.
    pub(crate) sectnumlevels: usize,

    /// Section type of outermost section. (Used to determine whether to number
    /// child sections as a normal section or appendix.)
    pub(crate) topmost_section_type: SectionType,

    /// Per-context counters for captioned blocks, keyed by counter name (e.g.
    /// `example-number`, `table-number`).
    ///
    /// Each captionable block context (example, table, …) maintains an
    /// independent, document-wide sequence. A counter is incremented each time
    /// a block of that context receives an automatically numbered caption
    /// (e.g. "Example 1.", "Table 1."). This mirrors Asciidoctor's
    /// per-context `Document#counters`.
    pub(crate) counters: HashMap<String, usize>,

    /// Canonical names of attributes that are locked against modification from
    /// the document body for the current scope.
    ///
    /// An AsciiDoc table cell creates a nested document that inherits the
    /// parent document's attributes. An attribute that is *set* in the
    /// parent _cannot_ be modified inside the cell (matching Asciidoctor,
    /// which here diverges from the spec's "set or explicitly unset" wording),
    /// so while a cell is being parsed every inherited attribute name
    /// (other than a handful of exceptions) is recorded here and a body
    /// attribute assignment to such a name is silently ignored. The set is
    /// saved and restored around each cell, so the lock applies only within
    /// the cell (and nests correctly).
    pub(crate) locked_attribute_names: HashSet<String>,

    /// Number of AsciiDoc table cells currently being parsed in the call stack.
    ///
    /// An AsciiDoc table cell creates a nested, standalone AsciiDoc document.
    /// While that document is being parsed this counter is greater than zero,
    /// which (matching Asciidoctor's `Document#nested?`) changes the default
    /// cell separator of any table found inside from the vertical bar (`|`) to
    /// the exclamation mark (`!`), so a nested table needs no explicit
    /// `separator` attribute. The counter is incremented and decremented around
    /// each AsciiDoc cell, so it nests correctly.
    pub(crate) nested_document_depth: usize,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            attribute_values: built_in_attrs(),
            default_attribute_values: built_in_default_values(),
            renderer: Rc::new(HtmlSubstitutionRenderer {}),
            primary_file_name: None,
            path_resolver: PathResolver::default(),
            include_file_handler: None,
            catalog: RefCell::new(Catalog::new()),
            last_section_number: SectionNumber::default(),
            last_appendix_section_number: SectionNumber {
                section_type: SectionType::Appendix,
                components: vec![],
            },
            sectnumlevels: 3,
            topmost_section_type: SectionType::Normal,
            counters: HashMap::new(),
            locked_attribute_names: HashSet::new(),
            nested_document_depth: 0,
        }
    }
}

impl Parser {
    /// Parse a UTF-8 string as an AsciiDoc document.
    ///
    /// The [`Document`] data structure returned by this call has a '`static`
    /// lifetime; this is an implementation detail. It retains a copy of the
    /// `source` string that was passed in, but it is not tied to the lifetime
    /// of that string.
    ///
    /// Nearly all of the data structures contained within the [`Document`]
    /// structure are tied to the lifetime of the document and have a `'src`
    /// lifetime to signal their dependency on the source document.
    ///
    /// **IMPORTANT:** The AsciiDoc language documentation states that UTF-16
    /// encoding is allowed if a byte-order-mark (BOM) is present at the
    /// start of a file. This format is not directly supported by the
    /// `asciidoc-parser` crate. Any UTF-16 content must be re-encoded as
    /// UTF-8 prior to parsing.
    ///
    /// The `Parser` struct will be updated with document attribute values
    /// discovered during parsing. These values may be inspected using
    /// [`attribute_value()`].
    ///
    /// # Warnings, not errors
    ///
    /// Any UTF-8 string is a valid AsciiDoc document, so this function does not
    /// return an [`Option`] or [`Result`] data type. There may be any number of
    /// character sequences that have ambiguous or potentially unintended
    /// meanings. For that reason, a caller is advised to review the warnings
    /// provided via the [`warnings()`] iterator.
    ///
    /// [`warnings()`]: Document::warnings
    /// [`attribute_value()`]: Self::attribute_value
    pub fn parse(&mut self, source: &str) -> Document<'static> {
        let mut document = self.parse_deferred(source);

        // Resolve cross-references against this document's own catalog. For
        // multi-document workflows, use `parse_deferred` and resolve later with
        // a caller-supplied resolver via `Document::resolve_references`.
        document.resolve_against_own_catalog(&*self.renderer);

        document
    }

    /// Parse a UTF-8 string as an AsciiDoc document, leaving cross-references
    /// unresolved.
    ///
    /// This behaves like [`parse()`], except it does not resolve
    /// cross-references (`<<id>>`, `xref:id[…]`). The returned [`Document`]
    /// carries its references in a deferred state; resolve them later with
    /// [`Document::resolve_references`].
    ///
    /// This is the entry point for multi-document workflows (e.g. Antora-style
    /// site generation): parse every document with this method, build a
    /// combined index from each document's [`catalog()`], then resolve each
    /// document against that index. This crate does not merge catalogs
    /// itself.
    ///
    /// [`parse()`]: Self::parse
    /// [`catalog()`]: Document::catalog
    pub fn parse_deferred(&mut self, source: &str) -> Document<'static> {
        let (preprocessed_source, source_map) = preprocess(source, self);

        // NOTE: `Document::parse` will transfer the catalog to itself at the end of the
        // parsing operation. Start each parse with a fresh catalog.
        *self.catalog.borrow_mut() = Catalog::new();

        // Reset section numbering for each new document.
        self.last_section_number = SectionNumber::default();

        // Reset captioned-block numbering for each new document.
        self.counters.clear();

        Document::parse(&preprocessed_source, source_map, self)
    }

    /// Retrieves the current interpreted value of a [document attribute].
    ///
    /// Each document holds a set of name-value pairs called document
    /// attributes. These attributes provide a means of configuring the AsciiDoc
    /// processor, declaring document metadata, and defining reusable content.
    /// This page introduces document attributes and answers some questions
    /// about the terminology used when referring to them.
    ///
    /// ## What are document attributes?
    ///
    /// Document attributes are effectively document-scoped variables for the
    /// AsciiDoc language. The AsciiDoc language defines a set of built-in
    /// attributes, and also allows the author (or extensions) to define
    /// additional document attributes, which may replace built-in attributes
    /// when permitted.
    ///
    /// Built-in attributes either provide access to read-only information about
    /// the document and its environment or allow the author to configure
    /// behavior of the AsciiDoc processor for a whole document or select
    /// regions. Built-in attributes are effectively unordered. User-defined
    /// attribute serve as a powerful text replacement tool. User-defined
    /// attributes are stored in the order in which they are defined.
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    pub fn attribute_value<N: AsRef<str>>(&self, name: N) -> InterpretedValue {
        self.attribute_values
            .get(name.as_ref())
            .map(|av| av.value.clone())
            .map(|av| {
                if let InterpretedValue::Set = av
                    && let Some(default) = self.default_attribute_values.get(name.as_ref())
                {
                    InterpretedValue::Value(default.clone())
                } else {
                    av
                }
            })
            .unwrap_or(InterpretedValue::Unset)
    }

    /// Returns `true` if the parser has a [document attribute] by this name.
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        self.attribute_values.contains_key(name.as_ref())
    }

    /// Returns `true` if the parser has a [document attribute] by this name
    /// which has been set (i.e. is present and not [unset]).
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        self.attribute_values
            .get(name.as_ref())
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or(false)
    }

    /// Resolves whether a document title should be displayed, from the
    /// `showtitle`/`notitle` attribute pair (which are complements).
    ///
    /// `showtitle` takes precedence: if present, the title shows precisely when
    /// it is set. Otherwise `notitle`, if present, hides the title when set.
    /// When neither attribute is present, `default_shown` decides — a
    /// standalone document (such as a nested AsciiDoc table cell) shows its
    /// title, while an embedded document does not.
    pub(crate) fn resolve_show_title(&self, default_shown: bool) -> bool {
        if self.has_attribute("showtitle") {
            self.is_attribute_set("showtitle")
        } else if self.has_attribute("notitle") {
            !self.is_attribute_set("notitle")
        } else {
            default_shown
        }
    }

    /// Forces the `doctype` attribute to `value`, refreshing the derived
    /// `backend-html5-doctype-*` attribute.
    ///
    /// Used when a nested AsciiDoc table cell resets its doctype to the default
    /// (a cell does not inherit the parent's doctype). The value stays
    /// modifiable from the document body so the cell may still set its own
    /// doctype.
    pub(crate) fn force_doctype(&mut self, value: &str) {
        self.attribute_values.insert(
            "doctype".to_string(),
            AttributeValue {
                allowable_value: AllowableValue::Any,
                modification_context: ModificationContext::ApiOrDocumentBody,
                value: InterpretedValue::Value(value.to_string()),
            },
        );
        self.refresh_doctype_derived_attr();
    }

    /// Recomputes the `backend-html5-doctype-{doctype}` intrinsic attribute so
    /// exactly one exists — for the active doctype — resolving to an empty
    /// (defined) value. References to any other doctype stay undefined and so
    /// render literally.
    pub(crate) fn refresh_doctype_derived_attr(&mut self) {
        self.attribute_values
            .retain(|name, _| !name.starts_with("backend-html5-doctype-"));

        if let InterpretedValue::Value(doctype) = self.attribute_value("doctype") {
            self.attribute_values.insert(
                format!("backend-html5-doctype-{doctype}"),
                AttributeValue {
                    allowable_value: AllowableValue::Any,
                    modification_context: ModificationContext::Anywhere,
                    value: InterpretedValue::Value(String::new()),
                },
            );
        }
    }

    /// Sets the value of an [intrinsic attribute].
    ///
    /// Intrinsic attributes are set automatically by the processor. These
    /// attributes provide information about the document being processed (e.g.,
    /// `docfile`), the security mode under which the processor is running
    /// (e.g., `safe-mode-name`), and information about the user’s environment
    /// (e.g., `user-home`).
    ///
    /// The [`modification_context`](ModificationContext) establishes whether
    /// the value can be subsequently modified by the document header and/or in
    /// the document body.
    ///
    /// Subsequent calls to this function or [`with_intrinsic_attribute_bool()`]
    /// are always permitted. The last such call for any given attribute name
    /// takes precendence.
    ///
    /// [intrinsic attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
    ///
    /// [`with_intrinsic_attribute_bool()`]: Self::with_intrinsic_attribute_bool
    pub fn with_intrinsic_attribute<N: AsRef<str>, V: AsRef<str>>(
        mut self,
        name: N,
        value: V,
        modification_context: ModificationContext,
    ) -> Self {
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            value: InterpretedValue::Value(value.as_ref().to_string()),
        };

        self.attribute_values
            .insert(name.as_ref().to_lowercase(), attribute_value);

        self
    }

    /// Register a referenceable element (anchor, section, bibliography entry)
    /// in the document catalog.
    ///
    /// This takes `&self` (rather than `&mut self`) so that it can be called
    /// from inline-substitution code paths that only hold a shared reference to
    /// the parser, such as a regex [`Replacer`](regex::Replacer).
    pub(crate) fn register_ref(
        &self,
        id: &str,
        reftext: Option<&str>,
        ref_type: RefType,
    ) -> Result<(), crate::document::DuplicateIdError> {
        self.catalog
            .borrow_mut()
            .register_ref(id, reftext, ref_type)
    }

    /// Generate a unique ID derived from `base_id` and register it in the
    /// document catalog, returning the ID that was assigned.
    pub(crate) fn generate_and_register_unique_id(
        &self,
        base_id: &str,
        reftext: Option<&str>,
        ref_type: RefType,
    ) -> String {
        self.catalog
            .borrow_mut()
            .generate_and_register_unique_id(base_id, reftext, ref_type)
    }

    /// Takes the catalog from the parser, transferring ownership and leaving an
    /// empty catalog in its place.
    ///
    /// This is used by `Document::parse` to transfer the catalog from the
    /// parser to the document at the end of parsing.
    pub(crate) fn take_catalog(&mut self) -> Catalog {
        std::mem::take(&mut *self.catalog.borrow_mut())
    }

    /* Comment out until we're prepared to use and test this.
        /// Sets the default value for an [intrinsic attribute].
        ///
        /// Default values for attributes are provided automatically by the
        /// processor. These values provide a falllback textual value for an
        /// attribute when it is merely "set" by the document via API, header, or
        /// document body.
        ///
        /// Calling this does not imply that the value is set automatically by
        /// default, nor does it establish any policy for where the value may be
        /// modified. For that, please use [`with_intrinsic_attribute`].
        ///
        /// [intrinsic attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
        /// [`with_intrinsic_attribute`]: Self::with_intrinsic_attribute
        pub fn with_default_attribute_value<N: AsRef<str>, V: AsRef<str>>(
            mut self,
            name: N,
            value: V,
        ) -> Self {
            self.default_attribute_values
                .insert(name.as_ref().to_string(), value.as_ref().to_string());

            self
        }
    */

    /// Sets the value of an [intrinsic attribute] from a boolean flag.
    ///
    /// A boolean `true` is interpreted as "set." A boolean `false` is
    /// interpreted as "unset."
    ///
    /// Intrinsic attributes are set automatically by the processor. These
    /// attributes provide information about the document being processed (e.g.,
    /// `docfile`), the security mode under which the processor is running
    /// (e.g., `safe-mode-name`), and information about the user’s environment
    /// (e.g., `user-home`).
    ///
    /// The [`modification_context`](ModificationContext) establishes whether
    /// the value can be subsequently modified by the document header and/or in
    /// the document body.
    ///
    /// Subsequent calls to this function or [`with_intrinsic_attribute()`] are
    /// always permitted. The last such call for any given attribute name takes
    /// precendence.
    ///
    /// [intrinsic attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
    ///
    /// [`with_intrinsic_attribute()`]: Self::with_intrinsic_attribute
    pub fn with_intrinsic_attribute_bool<N: AsRef<str>>(
        mut self,
        name: N,
        value: bool,
        modification_context: ModificationContext,
    ) -> Self {
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            value: if value {
                InterpretedValue::Set
            } else {
                InterpretedValue::Unset
            },
        };

        self.attribute_values
            .insert(name.as_ref().to_lowercase(), attribute_value);

        self
    }

    /// Replace the default [`InlineSubstitutionRenderer`] for this parser.
    ///
    /// The default implementation of [`InlineSubstitutionRenderer`] that is
    /// provided is suitable for HTML5 rendering. If you are targeting a
    /// different back-end rendering, you will need to provide your own
    /// implementation and set it using this call before parsing.
    pub fn with_inline_substitution_renderer<ISR: InlineSubstitutionRenderer + 'static>(
        mut self,
        renderer: ISR,
    ) -> Self {
        self.renderer = Rc::new(renderer);
        self
    }

    /// Sets the name of the primary file to be parsed when [`parse()`] is
    /// called.
    ///
    /// This name will be used for any error messages detected in this file and
    /// also will be passed to [`IncludeFileHandler::resolve_target()`] as the
    /// `source` argument for any `include::` file resolution requests from this
    /// file.
    ///
    /// [`parse()`]: Self::parse
    /// [`IncludeFileHandler::resolve_target()`]: crate::parser::IncludeFileHandler::resolve_target
    pub fn with_primary_file_name<S: AsRef<str>>(mut self, name: S) -> Self {
        self.primary_file_name = Some(name.as_ref().to_owned());
        self
    }

    /// Sets the [`IncludeFileHandler`] for this parser.
    ///
    /// The include file handler is responsible for resolving `include::`
    /// directives encountered during preprocessing. If no handler is provided,
    /// include directives will be ignored.
    ///
    /// [`IncludeFileHandler`]: crate::parser::IncludeFileHandler
    pub fn with_include_file_handler<IFH: IncludeFileHandler + 'static>(
        mut self,
        handler: IFH,
    ) -> Self {
        self.include_file_handler = Some(Rc::new(handler));
        self
    }

    /// Called from [`Header::parse()`] to accept or reject an attribute value.
    ///
    /// [`Header::parse()`]: crate::document::Header::parse
    pub(crate) fn set_attribute_from_header<'src>(
        &mut self,
        attr: &Attribute<'src>,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        let attr_name = remap_attr_name(attr.name().data());

        let existing_attr = self.attribute_values.get(&attr_name);

        // Verify that we have permission to overwrite any existing attribute value.
        if let Some(existing_attr) = existing_attr
            && (existing_attr.modification_context == ModificationContext::ApiOnly
                || existing_attr.modification_context == ModificationContext::ApiOrDocumentBody)
        {
            warnings.push(Warning {
                source: attr.span(),
                warning: WarningType::AttributeValueIsLocked(attr_name),
            });
            return;
        }

        let mut value = attr.value().clone();

        if let InterpretedValue::Set = value
            && let Some(default_value) = self.default_attribute_values.get(&attr_name)
        {
            value = InterpretedValue::Value(default_value.clone());
        }

        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value,
        };

        let is_doctype = attr_name == "doctype";
        self.attribute_values.insert(attr_name, attribute_value);
        if is_doctype {
            self.refresh_doctype_derived_attr();
        }
    }

    /// Called from [`Header::parse()`] for a value that is derived from parsing
    /// the header (except for attribute lines).
    ///
    /// [`Header::parse()`]: crate::document::Header::parse
    pub(crate) fn set_attribute_by_value_from_header<N: AsRef<str>, V: AsRef<str>>(
        &mut self,
        name: N,
        value: V,
    ) {
        let attr_name = remap_attr_name(name);

        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Value(value.as_ref().to_owned()),
        };

        self.attribute_values.insert(attr_name, attribute_value);
    }

    /// Called from [`Block::parse()`] to accept or reject an attribute value
    /// from a document (body) attribute.
    ///
    /// [`Block::parse()`]: crate::blocks::Block::parse
    pub(crate) fn set_attribute_from_body<'src>(
        &mut self,
        attr: &Attribute<'src>,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        let attr_name = remap_attr_name(attr.name().data());

        // An attribute inherited from the parent document of an AsciiDoc table
        // cell is locked for the duration of that cell: a body assignment to it
        // is silently ignored (no warning), matching Asciidoctor.
        if self.locked_attribute_names.contains(&attr_name) {
            return;
        }

        // Verify that we have permission to overwrite any existing attribute value.
        if let Some(existing_attr) = self.attribute_values.get(&attr_name)
            && (existing_attr.modification_context != ModificationContext::Anywhere
                && existing_attr.modification_context != ModificationContext::ApiOrDocumentBody)
        {
            warnings.push(Warning {
                source: attr.span(),
                warning: WarningType::AttributeValueIsLocked(attr_name),
            });
            return;
        }

        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: attr.value().clone(),
        };

        let is_doctype = attr_name == "doctype";
        self.attribute_values.insert(attr_name, attribute_value);
        if is_doctype {
            self.refresh_doctype_derived_attr();
        }
    }

    /// Assign the next section number for a given level.
    pub(crate) fn assign_section_number(&mut self, level: usize) -> SectionNumber {
        match self.topmost_section_type {
            SectionType::Normal => {
                self.last_section_number.assign_next_number(level);
                self.last_section_number.clone()
            }
            SectionType::Appendix => {
                self.last_appendix_section_number.assign_next_number(level);
                self.last_appendix_section_number.clone()
            }
            SectionType::Discrete => {
                // Shouldn't happen, but ignore if it does.
                self.last_section_number.clone()
            }
        }
    }

    /// Increments the document-wide counter named `name` and returns its new
    /// value.
    ///
    /// Captioned blocks are numbered in document order within their context,
    /// but only those that actually receive an automatically numbered caption
    /// consume a number. This mirrors Asciidoctor's
    /// `Document#increment_and_store_counter`.
    pub(crate) fn increment_counter(&mut self, name: &str) -> usize {
        let counter = self.counters.entry(name.to_string()).or_insert(0);
        *counter += 1;
        *counter
    }
}

fn remap_attr_name<N: AsRef<str>>(raw_attr_name: N) -> String {
    let attr_name = raw_attr_name.as_ref().to_lowercase();

    // Some attribute names have aliases. Remap to the primary name.
    match attr_name.as_str() {
        "hardbreaks" => "hardbreaks-option".to_string(),
        _ => attr_name,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        attributes::Attrlist,
        blocks::Block,
        parser::{
            CharacterReplacementType, IconRenderParams, ImageRenderParams,
            InlineSubstitutionRenderer, LinkRenderParams, QuoteScope, QuoteType, SpecialCharacter,
        },
        tests::prelude::*,
    };

    #[test]
    fn default_is_unset() {
        let p = Parser::default();
        assert_eq!(p.attribute_value("foo"), InterpretedValue::Unset);
    }

    #[test]
    fn creates_catalog_if_needed() {
        let mut p = Parser::default();
        let doc = p.parse("= Hello, World!\n\n== First Section Title");
        let cat = doc.catalog();
        assert!(cat.refs.contains_key("_first_section_title"));

        let doc = p.parse("= Hello, World!\n\n== Second Section Title");
        let cat = doc.catalog();
        assert!(!cat.refs.contains_key("_first_section_title"));
        assert!(cat.refs.contains_key("_second_section_title"));
    }

    #[test]
    fn with_intrinsic_attribute() {
        let p =
            Parser::default().with_intrinsic_attribute("foo", "bar", ModificationContext::Anywhere);

        assert_eq!(p.attribute_value("foo"), InterpretedValue::Value("bar"));
        assert_eq!(p.attribute_value("foo2"), InterpretedValue::Unset);

        assert!(p.is_attribute_set("foo"));
        assert!(!p.is_attribute_set("foo2"));
        assert!(!p.is_attribute_set("xyz"));
    }

    #[test]
    fn with_intrinsic_attribute_set() {
        let p = Parser::default().with_intrinsic_attribute_bool(
            "foo",
            true,
            ModificationContext::Anywhere,
        );

        assert_eq!(p.attribute_value("foo"), InterpretedValue::Set);
        assert_eq!(p.attribute_value("foo2"), InterpretedValue::Unset);

        assert!(p.is_attribute_set("foo"));
        assert!(!p.is_attribute_set("foo2"));
        assert!(!p.is_attribute_set("xyz"));
    }

    #[test]
    fn with_intrinsic_attribute_unset() {
        let p = Parser::default().with_intrinsic_attribute_bool(
            "foo",
            false,
            ModificationContext::Anywhere,
        );

        assert_eq!(p.attribute_value("foo"), InterpretedValue::Unset);
        assert_eq!(p.attribute_value("foo2"), InterpretedValue::Unset);

        assert!(!p.is_attribute_set("foo"));
        assert!(!p.is_attribute_set("foo2"));
        assert!(!p.is_attribute_set("xyz"));
    }

    #[test]
    fn can_not_override_locked_default_value() {
        let mut parser = Parser::default();

        let doc = parser.parse(":sp: not a space!");

        assert_eq!(
            doc.warnings().next().unwrap().warning,
            WarningType::AttributeValueIsLocked("sp".to_owned())
        );

        assert_eq!(parser.attribute_value("sp"), InterpretedValue::Value(" "));
    }

    #[test]
    fn catalog_transferred_to_document() {
        let mut parser = Parser::default();
        let doc = parser.parse("= Test Document\n\nSome content");

        let catalog = doc.catalog();
        assert!(catalog.is_empty());

        // The catalog was transferred to the document, leaving the parser with
        // an empty catalog.
        assert!(parser.catalog.borrow().is_empty());
    }

    #[test]
    fn block_ids_registered_in_catalog() {
        let mut parser = Parser::default();
        let doc = parser.parse("= Test Document\n\n[#my-block]\nSome content with an ID");

        let catalog = doc.catalog();
        assert!(!catalog.is_empty());
        assert!(catalog.contains_id("my-block"));

        let entry = catalog.get_ref("my-block").unwrap();
        assert_eq!(entry.id, "my-block");
        assert_eq!(entry.ref_type, crate::document::RefType::Anchor);
    }

    /// A simple test renderer that modifies special characters differently
    /// from the default HTML renderer.
    #[derive(Debug)]
    struct TestRenderer;

    impl InlineSubstitutionRenderer for TestRenderer {
        fn render_special_character(&self, type_: SpecialCharacter, dest: &mut String) {
            // Custom rendering: wrap special characters in brackets.
            match type_ {
                SpecialCharacter::Lt => dest.push_str("[LT]"),
                SpecialCharacter::Gt => dest.push_str("[GT]"),
                SpecialCharacter::Ampersand => dest.push_str("[AMP]"),
            }
        }

        fn render_quoted_substitition(
            &self,
            _type_: QuoteType,
            _scope: QuoteScope,
            _attrlist: Option<Attrlist<'_>>,
            _id: Option<String>,
            body: &str,
            dest: &mut String,
        ) {
            dest.push_str(body);
        }

        fn render_character_replacement(
            &self,
            _type_: CharacterReplacementType,
            dest: &mut String,
        ) {
            dest.push_str("[CHAR]");
        }

        fn render_line_break(&self, dest: &mut String) {
            dest.push_str("[BR]");
        }

        fn render_image(&self, _params: &ImageRenderParams, dest: &mut String) {
            dest.push_str("[IMAGE]");
        }

        fn image_uri(
            &self,
            target_image_path: &str,
            _parser: &Parser,
            _asset_dir_key: Option<&str>,
        ) -> String {
            target_image_path.to_string()
        }

        fn render_icon(&self, _params: &IconRenderParams, dest: &mut String) {
            dest.push_str("[ICON]");
        }

        fn render_link(&self, _params: &LinkRenderParams, dest: &mut String) {
            dest.push_str("[LINK]");
        }

        fn render_anchor(&self, id: &str, _reftext: Option<String>, dest: &mut String) {
            dest.push_str(&format!("[ANCHOR:{}]", id));
        }

        fn render_xref(&self, params: &crate::parser::XrefRenderParams, dest: &mut String) {
            dest.push_str(&format!("[XREF:{}]", params.target));
        }
    }

    #[test]
    fn with_inline_substitution_renderer() {
        let mut parser = Parser::default().with_inline_substitution_renderer(TestRenderer);

        // Parse a simple document with special characters.
        let doc = parser.parse("Hello & goodbye < world > test");

        // The document should parse successfully.
        assert_eq!(doc.warnings().count(), 0);

        // Get the first block from the document.
        let block = doc.nested_blocks().next().unwrap();

        let Block::Simple(simple_block) = block else {
            panic!("Expected simple block, got: {block:?}");
        };

        // Our custom renderer should show [AMP], [LT], and [GT] instead of HTML
        // entities.
        assert_eq!(
            simple_block.content().rendered(),
            "Hello [AMP] goodbye [LT] world [GT] test"
        );
    }

    mod resolve_show_title {
        use crate::parser::{ModificationContext, Parser};

        fn with(name: &str, set: bool) -> Parser {
            Parser::default().with_intrinsic_attribute_bool(
                name,
                set,
                ModificationContext::Anywhere,
            )
        }

        #[test]
        fn neither_present_uses_default() {
            assert!(Parser::default().resolve_show_title(true));
            assert!(!Parser::default().resolve_show_title(false));
        }

        #[test]
        fn showtitle_takes_precedence_and_decides() {
            // Present and set -> shown; present and unset -> hidden, regardless
            // of the default.
            assert!(with("showtitle", true).resolve_show_title(false));
            assert!(!with("showtitle", false).resolve_show_title(true));
        }

        #[test]
        fn notitle_is_the_complement_when_showtitle_absent() {
            // notitle set -> hidden; notitle unset -> shown.
            assert!(!with("notitle", true).resolve_show_title(true));
            assert!(with("notitle", false).resolve_show_title(false));
        }
    }

    mod refresh_doctype_derived_attr {
        use crate::{document::InterpretedValue, parser::Parser};

        #[test]
        fn tracks_the_active_doctype() {
            let mut parser = Parser::default();

            // The default doctype is `article`, so only its derived attribute is
            // defined (to an empty value).
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-article"),
                InterpretedValue::Value(String::new())
            );
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-book"),
                InterpretedValue::Unset
            );

            // Forcing a new doctype moves the derived attribute with it.
            parser.force_doctype("book");
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-book"),
                InterpretedValue::Value(String::new())
            );
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-article"),
                InterpretedValue::Unset
            );
        }

        #[test]
        fn defines_no_derived_attr_when_doctype_is_not_a_value() {
            let mut parser = Parser::default();

            // The default article derived attribute starts out defined.
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-article"),
                InterpretedValue::Value(String::new())
            );

            // With `doctype` unset (no `Value`), a refresh clears any existing
            // derived attribute and defines none.
            parser.attribute_values.remove("doctype");
            parser.refresh_doctype_derived_attr();

            assert_eq!(parser.attribute_value("doctype"), InterpretedValue::Unset);
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-article"),
                InterpretedValue::Unset
            );
        }
    }
}
