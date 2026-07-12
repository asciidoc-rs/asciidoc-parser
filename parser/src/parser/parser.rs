use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use crate::{
    Document, HasSpan,
    blocks::{SectionNumber, SectionType},
    document::{Attribute, Catalog, InterpretedValue, RefType},
    parser::{
        AllowableValue, AttributeValue, DocinfoFileHandler, HtmlSubstitutionRenderer,
        IncludeFileHandler, InlineSubstitutionRenderer, ModificationContext, PathResolver,
        ResolvedAttributes, SafeMode, SourceMap, SvgFileHandler,
        built_in_attrs::{built_in_attr, built_in_default_values, synthesized_attr},
        preprocessor::preprocess,
    },
    warnings::{Warning, WarningType},
};

/// The [`Parser`] struct and its related structs allow a caller to configure
/// how AsciiDoc parsing occurs and then to initiate the parsing process.
#[derive(Clone, Debug)]
pub struct Parser {
    /// Per-parser attribute values: **only** the attributes this parser has
    /// defined, overridden, or explicitly unset. The large set of built-in
    /// defaults is *not* copied in here; [`attribute_value`] falls back to the
    /// shared built-in table (see [`built_in_attrs`]) on a lookup miss, so
    /// creating or cloning a parser allocates nothing per built-in attribute.
    ///
    /// A per-parser entry always shadows the built-in default of the same name,
    /// including an [`Unset`](InterpretedValue::Unset) tombstone that records a
    /// built-in having been unset. The map is wrapped in an [`Arc`] so a parser
    /// clone (e.g. for a nested AsciiDoc table cell) shares it copy-on-write
    /// and only copies these (few) entries when it next modifies an
    /// attribute.
    ///
    /// [`attribute_value`]: Self::attribute_value
    /// [`built_in_attrs`]: super::built_in_attrs
    pub(crate) attribute_values: Arc<HashMap<String, AttributeValue>>,

    /// Default values for attributes if "set." Immutable after construction and
    /// shared via [`Arc`] (never copied per parser).
    default_attribute_values: Arc<HashMap<String, String>>,

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

    /// Handler for resolving docinfo files. If absent, no docinfo content is
    /// resolved.
    pub(crate) docinfo_file_handler: Option<Rc<dyn DocinfoFileHandler>>,

    /// Handler for reading the contents of an SVG file requested by an inline
    /// image with the `inline` option. If absent, inline SVG images fall back
    /// to rendering their alt text.
    pub(crate) svg_file_handler: Option<Rc<dyn SvgFileHandler>>,

    /// The safe mode under which the document is parsed and rendered. Controls
    /// security-sensitive rendering behavior (such as whether an interactive
    /// SVG image is rendered as an `<object>` element). Defaults to
    /// [`SafeMode::Secure`].
    pub(crate) safe: SafeMode,

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

    /// True while parsing the direct block children of a section that carries
    /// the `bibliography` style.
    ///
    /// A top-level unordered list parsed in this scope implicitly inherits the
    /// `bibliography` style (matching Asciidoctor), even without its own
    /// `[bibliography]` attribute. The flag is saved and restored around each
    /// section body, so a non-bibliography subsection clears it for its own
    /// children (the style does not propagate into subsections).
    pub(crate) parsing_bibliography_section_body: bool,

    /// True while the principal text of a bibliography list item is being
    /// substituted.
    ///
    /// Read through a shared `&Parser` by the macros substitution step so it
    /// recognizes a leading bibliography anchor (`[[[id]]]`). It is wrapped in
    /// a [`Cell`] because the substitution code paths (e.g. a regex
    /// [`Replacer`](regex::Replacer)) only hold a shared reference to the
    /// parser.
    pub(crate) in_bibliography_list_item: Cell<bool>,

    /// True while a footnote-free variant of some content is being substituted,
    /// so a `footnote:[…]` macro is dropped entirely rather than numbered and
    /// rendered as a marker. Used to derive a section title's reference text
    /// and auto-generated ID without the footnote leaking into either (see
    /// `SectionBlock::parse`).
    ///
    /// Wrapped in a [`Cell`] for the same reason as
    /// [`in_bibliography_list_item`](Self::in_bibliography_list_item): the
    /// substitution code paths hold only a shared reference to the parser.
    pub(crate) suppress_footnotes: Cell<bool>,

    /// Live values of [counter] attributes, keyed by counter name (e.g.
    /// `index`, `example-number`, `table-number`).
    ///
    /// A counter is a specialized document attribute: its value is *also* the
    /// value of the document attribute of the same name. Counters are resolved
    /// (and advanced) deep inside the attribute-reference substitution step,
    /// where only a shared `&Parser` is available, so the new value is recorded
    /// here through a [`RefCell`] and read back as an attribute by
    /// [`attribute_value()`]. An explicit attribute assignment to a counter's
    /// name supersedes this overlay (and is what allows `:!name:` to reset a
    /// counter), so every attribute setter clears the matching entry.
    ///
    /// Captioned blocks (example, table, …) are numbered with this same
    /// mechanism: each context's caption number is the counter named
    /// `<context>-number`, mirroring Asciidoctor's `Document#counter`.
    ///
    /// [counter]: https://docs.asciidoctor.org/asciidoc/latest/attributes/counters/
    /// [`attribute_value()`]: Self::attribute_value
    pub(crate) counter_values: RefCell<HashMap<String, String>>,

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

    /// Source map of the document currently being parsed, populated by
    /// [`Document::parse`] for the duration of the parse (and `None` outside
    /// it).
    ///
    /// Block parsing works from the *preprocessed* source, so a span's line
    /// number is relative to that flattened source rather than to the original
    /// input file(s). An AsciiDoc table cell whose first line is an `include::`
    /// directive re-runs the preprocessor over the cell's content: to report an
    /// unresolved directive against the file and line where it *originally*
    /// appeared (rather than "(root file)"), the cell must map its position in
    /// the preprocessed source back through this map. It is only consulted
    /// while parsing the top-level document (`nested_document_depth == 0`),
    /// where a cell's span still refers to that source.
    ///
    /// [`Document::parse`]: crate::Document
    pub(crate) source_map: Option<Rc<SourceMap>>,

    /// Number of include-expanded (owned) AsciiDoc table cells currently being
    /// parsed in the call stack.
    ///
    /// An AsciiDoc cell whose first line is an `include::` directive is parsed
    /// from a private, preprocessor-expanded copy of its content rather than
    /// from the document source. While that owned copy is being parsed this
    /// counter is greater than zero, and a span's line number no longer indexes
    /// the document [`source_map`](Self::source_map). A cell reached this way
    /// therefore cannot map its position back to an originating `(file, line)`,
    /// so it falls back to the root-file diagnostic. A cell nested inside a
    /// *borrowed* cell keeps document spans and is unaffected (the counter
    /// stays zero). The counter is incremented and decremented around each
    /// owned-cell parse, so it nests correctly.
    pub(crate) owned_cell_source_depth: usize,

    /// Catalog of callout numbers registered by verbatim blocks, used to
    /// validate the callout lists that annotate them.
    ///
    /// Wrapped in a [`RefCell`] because callouts are registered deep inside the
    /// callouts substitution step, where only a shared `&Parser` is available.
    callouts: RefCell<CalloutCatalog>,

    /// Warnings produced while replacing attribute references (e.g. a reference
    /// to a missing attribute when `attribute-missing` is `warn`).
    ///
    /// Wrapped in a [`RefCell`] because attribute references are replaced deep
    /// inside the attributes substitution step, where only a shared `&Parser`
    /// is available. Each entry stores the byte offset and length of the source
    /// span the warning refers to (rather than a borrowed
    /// [`Span`](crate::Span), which the lifetime-free `Parser` cannot
    /// hold), so the warnings can be turned into
    /// spanned [`Warning`]s once the document's owned source is available.
    substitution_warnings: RefCell<Vec<DeferredWarning>>,
}

/// A warning recorded in a form that does not borrow the source so it can live
/// on the [`Parser`] (or be returned from preprocessing), to be reconstituted
/// into a spanned [`Warning`] once the document's owned source is available.
///
/// This is used both for warnings raised while replacing attribute references
/// and for warnings raised during preprocessing (e.g. an unresolved include
/// directive). The `offset`/`len` pair locates the relevant text within the
/// (preprocessed) document source.
#[derive(Clone, Debug)]
pub(crate) struct DeferredWarning {
    /// Byte offset into the document source of the span this warning refers to.
    pub(crate) offset: usize,

    /// Byte length of the span this warning refers to.
    pub(crate) len: usize,

    /// The type of warning, already carrying any owned data it needs (such as
    /// the missing attribute's name).
    pub(crate) warning: WarningType,
}

/// Tracks the callout numbers defined by verbatim blocks so that a callout list
/// can be validated against the callouts it annotates.
///
/// This mirrors the relevant behavior of Asciidoctor's `Callouts` catalog: each
/// verbatim block registers the callout numbers it defines into the current
/// list, and each callout list checks its items against that list (warning
/// about any item with no matching callout) before the list is closed.
#[derive(Clone, Debug, Default)]
struct CalloutCatalog {
    /// Callout numbers registered (in document order) since the last callout
    /// list was closed.
    current: Vec<u32>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            // Starts empty: built-in defaults are resolved on the fly via the
            // shared table (see `attribute_value`), not copied in per parser.
            attribute_values: Arc::new(HashMap::new()),
            default_attribute_values: built_in_default_values(),
            renderer: Rc::new(HtmlSubstitutionRenderer {}),
            primary_file_name: None,
            path_resolver: PathResolver::default(),
            include_file_handler: None,
            docinfo_file_handler: None,
            svg_file_handler: None,
            safe: SafeMode::default(),
            catalog: RefCell::new(Catalog::new()),
            last_section_number: SectionNumber::default(),
            last_appendix_section_number: SectionNumber {
                section_type: SectionType::Appendix,
                components: vec![],
            },
            sectnumlevels: 3,
            topmost_section_type: SectionType::Normal,
            parsing_bibliography_section_body: false,
            in_bibliography_list_item: Cell::new(false),
            suppress_footnotes: Cell::new(false),
            counter_values: RefCell::new(HashMap::new()),
            locked_attribute_names: HashSet::new(),
            nested_document_depth: 0,
            source_map: None,
            owned_cell_source_depth: 0,
            callouts: RefCell::new(CalloutCatalog::default()),
            substitution_warnings: RefCell::new(vec![]),
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
        let (preprocessed_source, source_map, preprocessor_warnings) = preprocess(source, self);

        // NOTE: `Document::parse` will transfer the catalog to itself at the end of the
        // parsing operation. Start each parse with a fresh catalog.
        *self.catalog.borrow_mut() = Catalog::new();

        // Start each parse with an empty callout catalog.
        *self.callouts.borrow_mut() = CalloutCatalog::default();

        // Start each parse with no pending substitution warnings.
        self.substitution_warnings.borrow_mut().clear();

        // Reset section numbering for each new document.
        self.last_section_number = SectionNumber::default();

        // Reset counter (and captioned-block) numbering for each new document.
        self.counter_values.borrow_mut().clear();

        Document::parse(
            &preprocessed_source,
            source_map,
            preprocessor_warnings,
            self,
        )
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
        let name = name.as_ref();

        // A counter's current value lives in the overlay and supersedes any
        // earlier value of the attribute of the same name (see
        // [`counter_values`](Self::counter_values)).
        if let Some(value) = self.counter_values.borrow().get(name) {
            return InterpretedValue::Value(value.clone());
        }

        match self.effective_attribute(name) {
            Some(av) => {
                if let InterpretedValue::Set = av.value
                    && let Some(default) = self.default_attribute_values.get(name)
                {
                    InterpretedValue::Value(default.clone())
                } else {
                    av.value.clone()
                }
            }
            None => InterpretedValue::Unset,
        }
    }

    /// Returns the effective attribute definition for `name`: a per-parser
    /// entry (an override or an explicit [unset] tombstone) shadows the
    /// shared built-in default, which in turn shadows an on-the-fly
    /// synthesized attribute (the active `backend-html5-doctype-*` and
    /// `safe-mode-*` flags). The synthesized attributes are never
    /// materialized in either table.
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn effective_attribute(&self, name: &str) -> Option<&AttributeValue> {
        if let Some(av) = self.attribute_values.get(name) {
            return Some(av);
        }
        if let Some(av) = built_in_attr(name) {
            return Some(av);
        }
        synthesized_attr(name, &self.attribute_values)
    }

    /// Returns `true` if the parser has a [document attribute] by this name.
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        let name = name.as_ref();
        self.counter_values.borrow().contains_key(name) || self.effective_attribute(name).is_some()
    }

    /// Returns `true` if the parser has a [document attribute] by this name
    /// which has been set (i.e. is present and not [unset]).
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        let name = name.as_ref();

        // A counter always holds a concrete (set) value.
        if self.counter_values.borrow().contains_key(name) {
            return true;
        }

        self.effective_attribute(name)
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or(false)
    }

    /// Captures the parser's fully-resolved document-attribute state so it can
    /// outlive the parser — for example, retained on a [`Document`] to answer
    /// [`attribute_value`]/[`has_attribute`]/[`is_attribute_set`] without a
    /// parser in hand (the embed path a renderer uses for `convert_document`).
    ///
    /// This shares the parser's attribute tables by [`Arc`] rather than copying
    /// them, so it is cheap to take on every parse (the large built-in table is
    /// never deep-cloned). See [`ResolvedAttributes`].
    ///
    /// [`Document`]: crate::Document
    /// [`attribute_value`]: Self::attribute_value
    /// [`has_attribute`]: Self::has_attribute
    /// [`is_attribute_set`]: Self::is_attribute_set
    pub(crate) fn snapshot_attributes(&self) -> ResolvedAttributes {
        ResolvedAttributes::new(
            Arc::clone(&self.attribute_values),
            Arc::clone(&self.default_attribute_values),
            self.counter_values.borrow().clone(),
        )
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

    /// Forces the `doctype` attribute to `value`.
    ///
    /// Used when a nested AsciiDoc table cell resets its doctype to the default
    /// (a cell does not inherit the parent's doctype). The value stays
    /// modifiable from the document body so the cell may still set its own
    /// doctype.
    ///
    /// The derived `backend-html5-doctype-{doctype}` attribute needs no
    /// explicit refresh: it is synthesized on the fly for whatever
    /// `doctype` currently resolves to (see
    /// [`attribute_value`](Self::attribute_value)).
    pub(crate) fn force_doctype(&mut self, value: &str) {
        Arc::make_mut(&mut self.attribute_values).insert(
            "doctype".to_string(),
            AttributeValue {
                allowable_value: AllowableValue::Any,
                modification_context: ModificationContext::ApiOrDocumentBody,
                silent_when_locked: false,
                value: InterpretedValue::Value(value.to_string()),
            },
        );
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
            silent_when_locked: false,
            value: InterpretedValue::Value(value.as_ref().to_string()),
        };

        Arc::make_mut(&mut self.attribute_values)
            .insert(name.as_ref().to_lowercase(), attribute_value);

        self
    }

    /// Sets the value of an [intrinsic attribute], rejecting any disallowed
    /// subsequent write *silently*.
    ///
    /// This behaves exactly like [`with_intrinsic_attribute()`] except that a
    /// document header or body assignment that the
    /// [`modification_context`](ModificationContext) does not permit is dropped
    /// with **no** `AttributeValueIsLocked` warning, instead of recording one.
    /// The rejected write is otherwise handled identically (the value is left
    /// unchanged).
    ///
    /// This reproduces Asciidoctor's *silent* safe-mode attribute restrictions:
    /// under `SERVER`/`SECURE`, a document assignment of a restricted
    /// conversion attribute (`backend`, `doctype`, `docinfo`,
    /// `source-highlighter`) is simply dropped, with no diagnostic. Seed
    /// such an attribute as an [`ApiOnly`](ModificationContext::ApiOnly)
    /// silent intrinsic to lock it against document assignment without
    /// warning.
    ///
    /// Subsequent calls to this function or the other
    /// `with_intrinsic_attribute` variants are always permitted. The last
    /// such call for any given attribute name takes precedence.
    ///
    /// [intrinsic attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
    ///
    /// [`with_intrinsic_attribute()`]: Self::with_intrinsic_attribute
    pub fn with_intrinsic_attribute_silent<N: AsRef<str>, V: AsRef<str>>(
        mut self,
        name: N,
        value: V,
        modification_context: ModificationContext,
    ) -> Self {
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: true,
            value: InterpretedValue::Value(value.as_ref().to_string()),
        };

        Arc::make_mut(&mut self.attribute_values)
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

    /// Attaches an [`XrefSignifier`](crate::parser::XrefSignifier) to an
    /// already-registered catalog element, so a cross-reference to it can build
    /// `full`/`short` [`xrefstyle`](crate::parser::XrefStyle) text.
    ///
    /// Takes `&self` for the same reason as
    /// [`register_ref`](Self::register_ref).
    pub(crate) fn set_ref_signifier(&self, id: &str, signifier: crate::parser::XrefSignifier) {
        self.catalog.borrow_mut().set_signifier(id, signifier);
    }

    /// Registers a callout number defined by a verbatim block.
    ///
    /// Takes `&self` so it can be called from the callouts substitution step,
    /// which only holds a shared reference to the parser.
    pub(crate) fn register_callout(&self, number: u32) {
        self.callouts.borrow_mut().current.push(number);
    }

    /// Returns `true` if a callout numbered `number` was registered for the
    /// current (not-yet-closed) callout list.
    pub(crate) fn callout_defined(&self, number: u32) -> bool {
        self.callouts.borrow().current.contains(&number)
    }

    /// Closes the current callout list, so callouts registered afterward belong
    /// to the next list.
    pub(crate) fn close_callout_list(&self) {
        self.callouts.borrow_mut().current.clear();
    }

    /// Returns the number of an already-defined footnote with the given ID, if
    /// one exists in the current document's footnote registry.
    ///
    /// Takes `&self` so it can be called from the macros substitution step,
    /// which only holds a shared reference to the parser.
    pub(crate) fn footnote_index_for_id(&self, id: &str) -> Option<String> {
        self.catalog
            .borrow()
            .footnote_with_id(id)
            .map(|f| f.index.clone())
    }

    /// Defines a new footnote, advancing the `footnote-number` counter and
    /// registering the footnote in the current document's registry. Returns the
    /// number assigned to the footnote.
    ///
    /// Takes `&self` so it can be called from the macros substitution step.
    pub(crate) fn define_footnote(
        &self,
        id: Option<&str>,
        text: String,
        xrefs: Vec<crate::content::XrefSegment>,
    ) -> String {
        // A footnote's text is extracted out of the block during macro
        // substitution, so any cross-reference inside it never reaches the
        // document-level resolution pass over block content. Those
        // cross-references are captured (as placeholders in `text` plus the
        // `xrefs` segments) so they can be resolved alongside the block
        // references. The stored `text` is the unresolved fallback rendering
        // until then, so it is always clean.
        let (text, deferred) = if xrefs.is_empty() {
            (text, None)
        } else {
            let deferred = crate::content::FootnoteDeferred::new(text, xrefs);
            let rendered = deferred.render(&*self.renderer);
            (rendered, Some(Box::new(deferred)))
        };

        // Footnotes are numbered consecutively throughout the document via the
        // `footnote-number` counter, which is seeded to `0` so the first
        // footnote is numbered `1`. The counter is a document-wide attribute, so
        // numbering continues across nested documents (AsciiDoc table cells)
        // even though the footnote *list* does not. The counter honors any seed
        // the document sets, so a non-integer seed yields a non-integer number
        // (matching Asciidoctor); the value is therefore kept as a string.
        let index = self.counter("footnote-number", None);

        self.catalog
            .borrow_mut()
            .register_footnote(crate::document::Footnote {
                index: index.clone(),
                id: id.map(|s| s.to_owned()),
                text,
                deferred,
            });

        index
    }

    /// Removes and returns the current document's footnote list, leaving an
    /// empty list behind. Used to give a nested document (an AsciiDoc table
    /// cell) its own footnote registry; see [`restore_footnotes`].
    ///
    /// [`restore_footnotes`]: Self::restore_footnotes
    pub(crate) fn take_footnotes(&self) -> Vec<crate::document::Footnote> {
        self.catalog.borrow_mut().take_footnotes()
    }

    /// Restores a previously-[taken](Self::take_footnotes) footnote list,
    /// discarding any footnotes registered in the meantime (i.e. those defined
    /// inside the nested document).
    pub(crate) fn restore_footnotes(&self, footnotes: Vec<crate::document::Footnote>) {
        self.catalog.borrow_mut().restore_footnotes(footnotes);
    }

    /// Records a warning produced while replacing attribute references.
    ///
    /// Takes `&self` so it can be called from the attributes substitution step,
    /// which only holds a shared reference to the parser. `source` locates the
    /// text the warning refers to; its byte offset and length are stored so a
    /// spanned [`Warning`] can be reconstructed later (see
    /// [`take_substitution_warnings`](Self::take_substitution_warnings)).
    pub(crate) fn record_substitution_warning(
        &self,
        source: crate::Span<'_>,
        warning: WarningType,
    ) {
        self.substitution_warnings
            .borrow_mut()
            .push(DeferredWarning {
                offset: source.byte_offset(),
                len: source.len(),
                warning,
            });
    }

    /// Returns the number of substitution warnings recorded so far.
    ///
    /// Used together with [`truncate_substitution_warnings`] to discard
    /// warnings recorded while parsing an owned (e.g. include-expanded) source,
    /// whose offsets do not refer to the primary document source.
    ///
    /// [`truncate_substitution_warnings`]: Self::truncate_substitution_warnings
    pub(crate) fn substitution_warnings_len(&self) -> usize {
        self.substitution_warnings.borrow().len()
    }

    /// Discards any substitution warnings recorded since the buffer held `len`
    /// entries.
    pub(crate) fn truncate_substitution_warnings(&self, len: usize) {
        self.substitution_warnings.borrow_mut().truncate(len);
    }

    /// Takes the substitution warnings recorded during parsing, leaving the
    /// buffer empty.
    pub(crate) fn take_substitution_warnings(&self) -> Vec<DeferredWarning> {
        std::mem::take(&mut *self.substitution_warnings.borrow_mut())
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
            silent_when_locked: false,
            value: if value {
                InterpretedValue::Set
            } else {
                InterpretedValue::Unset
            },
        };

        Arc::make_mut(&mut self.attribute_values)
            .insert(name.as_ref().to_lowercase(), attribute_value);

        self
    }

    /// Sets the value of an [intrinsic attribute] from a boolean flag,
    /// rejecting any disallowed subsequent write *silently*.
    ///
    /// This behaves exactly like [`with_intrinsic_attribute_bool()`] except
    /// that a document header or body assignment that the
    /// [`modification_context`](ModificationContext) does not permit is dropped
    /// with **no** `AttributeValueIsLocked` warning, instead of recording one.
    /// See [`with_intrinsic_attribute_silent()`] for the motivating use case
    /// (Asciidoctor's silent safe-mode attribute restrictions).
    ///
    /// A boolean `true` is interpreted as "set." A boolean `false` is
    /// interpreted as "unset."
    ///
    /// Subsequent calls to this function or the other
    /// `with_intrinsic_attribute` variants are always permitted. The last
    /// such call for any given attribute name takes precedence.
    ///
    /// [intrinsic attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
    ///
    /// [`with_intrinsic_attribute_bool()`]: Self::with_intrinsic_attribute_bool
    /// [`with_intrinsic_attribute_silent()`]: Self::with_intrinsic_attribute_silent
    pub fn with_intrinsic_attribute_bool_silent<N: AsRef<str>>(
        mut self,
        name: N,
        value: bool,
        modification_context: ModificationContext,
    ) -> Self {
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: true,
            value: if value {
                InterpretedValue::Set
            } else {
                InterpretedValue::Unset
            },
        };

        Arc::make_mut(&mut self.attribute_values)
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

    /// Sets the [`DocinfoFileHandler`] for this parser.
    ///
    /// The docinfo file handler is responsible for providing the content of
    /// [docinfo files] requested while resolving a document's docinfo (see the
    /// `docinfo` attribute). If no handler is provided, no docinfo content is
    /// resolved and [`Document::docinfo`] returns an empty string for every
    /// location.
    ///
    /// [`DocinfoFileHandler`]: crate::parser::DocinfoFileHandler
    /// [docinfo files]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
    /// [`Document::docinfo`]: crate::Document::docinfo
    pub fn with_docinfo_file_handler<DFH: DocinfoFileHandler + 'static>(
        mut self,
        handler: DFH,
    ) -> Self {
        self.docinfo_file_handler = Some(Rc::new(handler));
        self
    }

    /// Sets the [`SvgFileHandler`] for this parser.
    ///
    /// The SVG file handler is responsible for providing the raw contents of an
    /// SVG file requested by an inline image with the `inline` option (e.g.
    /// `image:diagram.svg[opts=inline]`). If no handler is provided, inline SVG
    /// images fall back to rendering their alt text.
    ///
    /// [`SvgFileHandler`]: crate::parser::SvgFileHandler
    pub fn with_svg_file_handler<SFH: SvgFileHandler + 'static>(mut self, handler: SFH) -> Self {
        self.svg_file_handler = Some(Rc::new(handler));
        self
    }

    /// Sets the [`SafeMode`] under which the document is parsed and rendered.
    ///
    /// The default is [`SafeMode::Secure`], the most conservative setting.
    /// Relaxing the safe mode enables security-sensitive rendering behavior,
    /// such as rendering an interactive SVG image as an `<object>` element.
    ///
    /// [`SafeMode`]: crate::SafeMode
    pub fn with_safe_mode(mut self, safe: SafeMode) -> Self {
        self.safe = safe;
        self.apply_safe_mode_attributes();
        self
    }

    /// Overrides the `safe-mode-*` family of [intrinsic attributes] from the
    /// current safe mode.
    ///
    /// These attributes let a document (or a downstream converter) inspect the
    /// security mode under which it is being processed:
    ///
    /// * `safe-mode-level` — the numeric level (`0`, `1`, `10`, or `20`).
    /// * `safe-mode-name` — the lowercase mode name (`unsafe`, `safe`,
    ///   `server`, or `secure`).
    /// * `safe-mode-<name>` — a single flag attribute (set to an empty value)
    ///   naming the active mode; the flags for the other modes are absent so
    ///   that a reference to them resolves literally.
    ///
    /// Only `safe-mode-level` and `safe-mode-name` are stored here (shadowing
    /// their built-in Secure-mode defaults). The active `safe-mode-<name>` flag
    /// is synthesized on the fly from `safe-mode-name` (see
    /// [`synthesized_attr`]), so exactly one flag is ever defined and the
    /// inactive flags stay absent without any per-mode bookkeeping here.
    ///
    /// All of these are read-only from the document's perspective (they can
    /// only be established via the API), matching Ruby Asciidoctor.
    ///
    /// [intrinsic attributes]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes-ref/#intrinsic-attributes
    fn apply_safe_mode_attributes(&mut self) {
        let intrinsic = |value: InterpretedValue| AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            silent_when_locked: false,
            value,
        };

        let attrs = Arc::make_mut(&mut self.attribute_values);
        attrs.insert(
            "safe-mode-level".to_string(),
            intrinsic(InterpretedValue::Value(self.safe.level().to_string())),
        );
        attrs.insert(
            "safe-mode-name".to_string(),
            intrinsic(InterpretedValue::Value(self.safe.name().to_string())),
        );
    }

    /// Returns the [`SafeMode`] under which this parser operates.
    ///
    /// [`SafeMode`]: crate::SafeMode
    pub fn safe_mode(&self) -> SafeMode {
        self.safe
    }

    /// Returns the document name (`docname`): the base name of the primary
    /// file, stripped of its directory and final extension.
    ///
    /// This is the `<docname>` used to build private docinfo file names (e.g.
    /// `mydoc-docinfo.html` for `mydoc.adoc`). Returns `None` when no primary
    /// file name has been set, in which case private docinfo files cannot be
    /// resolved.
    pub(crate) fn docname(&self) -> Option<String> {
        let primary = self.primary_file_name.as_deref()?;

        // Strip the directory portion (handling both separators, since the
        // primary file name may have been supplied on either platform).
        let base = primary.rsplit(['/', '\\']).next().unwrap_or(primary);

        // Strip a single trailing extension, if present. A leading-dot name
        // (e.g. `.adoc`) is treated as having no extension and is kept whole as
        // the stem, matching Ruby's `File.basename(".adoc", ".*")`.
        let stem = match base.rfind('.') {
            Some(0) | None => base,
            Some(idx) => &base[..idx],
        };

        if stem.is_empty() {
            None
        } else {
            Some(stem.to_string())
        }
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

        // The `backend-html5-doctype-*` namespace is a read-only synthesized
        // intrinsic; a document must not write any of it (see
        // [`is_reserved_doctype_derived_attr`]).
        if is_reserved_doctype_derived_attr(&attr_name) {
            return;
        }

        // Verify that we have permission to overwrite any existing attribute
        // value, considering both a per-parser entry and the shared built-in
        // default it would shadow (a built-in such as `sp` is `ApiOnly`).
        if let Some(existing_attr) = self.effective_attribute(&attr_name)
            && (existing_attr.modification_context == ModificationContext::ApiOnly
                || existing_attr.modification_context == ModificationContext::ApiOrDocumentBody)
        {
            // A silently-locked intrinsic rejects the write without recording a
            // warning (see `AttributeValue::silent_when_locked`).
            if !existing_attr.silent_when_locked {
                warnings.push(Warning {
                    source: attr.span(),
                    warning: WarningType::AttributeValueIsLocked(attr_name),
                });
            }
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
            silent_when_locked: false,
            value,
        };

        // An explicit assignment supersedes (and resets) any counter of the same
        // name.
        self.counter_values.borrow_mut().remove(&attr_name);

        // The derived `backend-html5-doctype-*` attribute tracks `doctype`
        // automatically (it is synthesized on lookup), so no refresh is needed.
        Arc::make_mut(&mut self.attribute_values).insert(attr_name, attribute_value);
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
            silent_when_locked: false,
            value: InterpretedValue::Value(value.as_ref().to_owned()),
        };

        self.counter_values.borrow_mut().remove(&attr_name);
        Arc::make_mut(&mut self.attribute_values).insert(attr_name, attribute_value);
    }

    /// Applies the `imagesdir`-relative default for the `iconsdir` attribute.
    ///
    /// The `iconsdir` attribute defaults to `{imagesdir}/icons`; when
    /// `imagesdir` is left empty this resolves to the built-in
    /// [`DEFAULT_ICONSDIR`] (`./images/icons`). When `imagesdir` is set to a
    /// non-empty value and `iconsdir` was left at its built-in default, the
    /// icons directory is derived as `{imagesdir}/icons`.
    ///
    /// The derivation is skipped — so an explicit `iconsdir` wins — when either
    /// the attribute was set in the header (`iconsdir_set_in_header`) or its
    /// resolved value differs from [`DEFAULT_ICONSDIR`] (which is how an
    /// override applied any other way, e.g. via the API, is detected). The one
    /// case this cannot detect is a non-header override whose value happens to
    /// equal the built-in default (e.g. an API caller setting `iconsdir` to
    /// exactly `./images/icons`): it is indistinguishable from the default and
    /// so is re-derived. That combination is contradictory in practice (it
    /// pins `iconsdir` to the value it would take were `imagesdir` unset) and
    /// is not worth a dedicated provenance flag.
    ///
    /// This is called once, after the document header is parsed, mirroring
    /// Asciidoctor's document-initialization timing (a later `imagesdir` change
    /// in the document body does not retroactively re-derive `iconsdir`). See
    /// icons-image.adoc.
    ///
    /// [`DEFAULT_ICONSDIR`]: super::built_in_attrs::DEFAULT_ICONSDIR
    pub(crate) fn apply_iconsdir_default(&mut self, iconsdir_set_in_header: bool) {
        if iconsdir_set_in_header {
            return;
        }

        // Preserve any override whose value differs from the built-in default
        // (e.g. one applied via the API); only the built-in default itself is
        // eligible for `imagesdir`-relative derivation. See the doc comment for
        // the one indistinguishable corner case.
        if self.attribute_value("iconsdir").as_maybe_str()
            != Some(super::built_in_attrs::DEFAULT_ICONSDIR)
        {
            return;
        }

        let imagesdir = self.attribute_value("imagesdir");
        let derived = match imagesdir.as_maybe_str().filter(|d| !d.is_empty()) {
            Some(dir) => format!("{}/icons", dir.trim_end_matches('/')),
            None => return,
        };

        self.set_attribute_by_value_from_header("iconsdir", derived);
    }

    /// Called while parsing a block (see [`Block::parse_with_outcome()`]) to
    /// accept or reject an attribute value from a document (body) attribute.
    ///
    /// [`Block::parse_with_outcome()`]: crate::blocks::Block::parse_with_outcome
    pub(crate) fn set_attribute_from_body<'src>(
        &mut self,
        attr: &Attribute<'src>,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        let attr_name = remap_attr_name(attr.name().data());

        // The `backend-html5-doctype-*` namespace is a read-only synthesized
        // intrinsic; a document must not write any of it (see
        // [`is_reserved_doctype_derived_attr`]).
        if is_reserved_doctype_derived_attr(&attr_name) {
            return;
        }

        // An attribute inherited from the parent document of an AsciiDoc table
        // cell is locked for the duration of that cell: a body assignment to it
        // is silently ignored (no warning), matching Asciidoctor.
        if self.locked_attribute_names.contains(&attr_name) {
            return;
        }

        // Verify that we have permission to overwrite any existing attribute
        // value, considering both a per-parser entry and the shared built-in
        // default it would shadow.
        if let Some(existing_attr) = self.effective_attribute(&attr_name)
            && (existing_attr.modification_context != ModificationContext::Anywhere
                && existing_attr.modification_context != ModificationContext::ApiOrDocumentBody)
        {
            // A silently-locked intrinsic rejects the write without recording a
            // warning (see `AttributeValue::silent_when_locked`).
            if !existing_attr.silent_when_locked {
                warnings.push(Warning {
                    source: attr.span(),
                    warning: WarningType::AttributeValueIsLocked(attr_name),
                });
            }
            return;
        }

        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            silent_when_locked: false,
            value: attr.value().clone(),
        };

        // An explicit assignment supersedes (and resets) any counter of the same
        // name. This is what lets `:!name:` reset a counter.
        self.counter_values.borrow_mut().remove(&attr_name);

        // The derived `backend-html5-doctype-*` attribute tracks `doctype`
        // automatically (it is synthesized on lookup), so no refresh is needed.
        Arc::make_mut(&mut self.attribute_values).insert(attr_name, attribute_value);
    }

    /// Assign the next section number for a given level.
    pub(crate) fn assign_section_number(&mut self, level: usize) -> SectionNumber {
        match self.topmost_section_type {
            SectionType::Appendix => {
                self.last_appendix_section_number.assign_next_number(level);
                self.last_appendix_section_number.clone()
            }

            // `topmost_section_type` is only ever `Normal` or `Appendix`: a
            // discrete heading never becomes the topmost section type (see
            // `SectionBlock::parse`). `Discrete` therefore cannot reach this
            // point, so it is folded in with `Normal` rather than carried as a
            // separate, untestable arm.
            SectionType::Normal | SectionType::Discrete => {
                self.last_section_number.assign_next_number(level);
                self.last_section_number.clone()
            }
        }
    }

    /// Resolves a [counter] of the given `name`, advancing it to the next value
    /// in its sequence and returning that value.
    ///
    /// A counter is a specialized document attribute: its value is stored as
    /// (and read back from) the attribute of the same name, so a later
    /// `{name}` reference shows the current value and an attribute assignment
    /// such as `:!name:` resets it. Each resolution advances the counter:
    ///
    /// * an integer value is incremented (`1` -> `2`);
    /// * any other value is advanced like Ruby's `String#succ` (`a` -> `b`, `z`
    ///   -> `aa`, `Az` -> `Ba`), matching Asciidoctor.
    ///
    /// `seed` (from the `{counter:name:seed}` form) supplies the first value,
    /// but only when the counter is currently unset; otherwise it is ignored.
    /// With no seed the sequence starts at `1`.
    ///
    /// This mirrors Asciidoctor's `Document#counter`.
    ///
    /// [counter]: https://docs.asciidoctor.org/asciidoc/latest/attributes/counters/
    pub(crate) fn counter(&self, name: &str, seed: Option<&str>) -> String {
        let next = match self.attribute_value(name) {
            InterpretedValue::Value(current) if !current.is_empty() => next_counter_value(&current),
            _ => match seed {
                Some(seed) if !seed.is_empty() => seed.to_string(),
                _ => "1".to_string(),
            },
        };

        self.counter_values
            .borrow_mut()
            .insert(name.to_string(), next.clone());

        next
    }
}

/// Advances a counter value to the next value in its sequence, mirroring
/// Asciidoctor's `Helpers.nextval`.
///
/// A canonical integer string (one that round-trips through integer parsing,
/// e.g. `7` but not `07` or `+7`) is incremented numerically. Anything else is
/// advanced with [`string_succ`].
fn next_counter_value(current: &str) -> String {
    if let Ok(n) = current.parse::<i64>()
        && n.to_string() == current
    {
        // `saturating_add` keeps a counter that has somehow reached `i64::MAX`
        // pinned there rather than panicking (debug) or wrapping (release).
        return n.saturating_add(1).to_string();
    }

    string_succ(current)
}

/// Returns the successor of a string, mirroring Ruby's `String#succ` for the
/// ASCII cases that AsciiDoc counters can produce.
///
/// The right-most alphanumeric character is incremented within its own class
/// (digits, lowercase letters, uppercase letters), carrying leftward on
/// wrap-around (`9` -> `0`, `z` -> `a`, `Z` -> `A`) and prepending a fresh
/// leading character (`1`, `a`, or `A`) when the carry runs off the front
/// (`z` -> `aa`, `Zz` -> `AAa`). A string with no alphanumeric characters has
/// the code point of its last character incremented.
fn string_succ(current: &str) -> String {
    let chars: Vec<char> = current.chars().collect();

    // Without an alphanumeric to carry through, Ruby increments the code point
    // of the final character.
    if !chars.iter().any(char::is_ascii_alphanumeric) {
        let mut chars = chars;
        if let Some(last) = chars.last_mut() {
            *last = char::from_u32(*last as u32 + 1).unwrap_or(*last);
        }
        return chars.into_iter().collect();
    }

    // Walk right to left. `carrying` stays true while we are still looking for
    // (or carrying through) the alphanumeric run: trailing non-alphanumeric
    // characters are passed over unchanged, then the right-most alphanumeric is
    // incremented within its class and any wrap-around carries leftward to the
    // next alphanumeric. When the carry runs off the front, a fresh leading
    // character of the same class is prepended (`z` -> `aa`, `9` -> `10`).
    let mut out_rev: Vec<char> = Vec::with_capacity(chars.len() + 1);
    let mut carrying = true;
    let mut lead = '1';

    for &c in chars.iter().rev() {
        if carrying && c.is_ascii_alphanumeric() {
            // Increment within the character's class, carrying on wrap-around.
            // The arms are exhaustive over ASCII alphanumerics, so the catch-all
            // can only be `Z` (the one value not matched above).
            let (next, carry) = match c {
                '0'..='8' | 'a'..='y' | 'A'..='Y' => ((c as u8 + 1) as char, false),
                '9' => ('0', true),
                'z' => ('a', true),
                _ => ('A', true),
            };
            out_rev.push(next);
            carrying = carry;
            // On a carry, remember the class of leading character to prepend if
            // the carry runs off the front; `next` is `0`, `a`, or `A` here.
            lead = match next {
                '0' => '1',
                'a' => 'a',
                _ => 'A',
            };
        } else {
            // Either the carry is spent, or this is a trailing non-alphanumeric
            // we pass over while still searching for the run to increment.
            out_rev.push(c);
        }
    }

    if carrying {
        out_rev.push(lead);
    }

    out_rev.into_iter().rev().collect()
}

fn remap_attr_name<N: AsRef<str>>(raw_attr_name: N) -> String {
    let attr_name = raw_attr_name.as_ref().to_lowercase();

    // Some attribute names have aliases. Remap to the primary name.
    match attr_name.as_str() {
        "hardbreaks" => "hardbreaks-option".to_string(),
        _ => attr_name,
    }
}

/// Returns `true` if `name` belongs to the reserved `backend-html5-doctype-*`
/// namespace, which is a read-only synthesized intrinsic keyed on the active
/// `doctype` (see [`synthesized_attr`]).
///
/// A document header or body assignment to any such name is rejected — not only
/// the flag that is active when the assignment is parsed. Otherwise a name that
/// is inactive at assignment time (e.g. `backend-html5-doctype-article` while
/// the doctype is `book`) would resolve to no synthesized attribute, pass the
/// permission check, and be stored as a per-parser override that then shadows
/// the intrinsic once the doctype switches to that value (e.g. in an AsciiDoc
/// table cell that resets, then changes, its doctype).
fn is_reserved_doctype_derived_attr(name: &str) -> bool {
    name.starts_with("backend-html5-doctype-")
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
    fn silently_locked_intrinsic_rejects_header_and_body_without_warning() {
        // A silently-locked `ApiOnly` intrinsic (as a converter would seed a
        // safe-mode-restricted attribute) rejects both a header assignment and a
        // body assignment of the same name, leaving the value unchanged and
        // recording no warning.
        let mut parser = Parser::default().with_intrinsic_attribute_silent(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        let doc = parser.parse(concat!(
            "= Title\n",
            ":backend: docbook5\n",
            "\n",
            "Body paragraph.\n",
            "\n",
            ":backend: manpage\n",
        ));

        assert_eq!(doc.warnings().count(), 0);
        assert_eq!(
            parser.attribute_value("backend"),
            InterpretedValue::Value("html5")
        );
    }

    #[test]
    fn silently_locked_bool_intrinsic_rejects_without_warning() {
        let mut parser = Parser::default().with_intrinsic_attribute_bool_silent(
            "sectids",
            true,
            ModificationContext::ApiOnly,
        );

        let doc = parser.parse(concat!("= Title\n", ":!sectids:\n"));

        assert_eq!(doc.warnings().count(), 0);
        assert_eq!(parser.attribute_value("sectids"), InterpretedValue::Set);
    }

    #[test]
    fn silently_locked_bool_intrinsic_false_is_unset() {
        // A `false` flag records an `Unset` tombstone, and a locked (`ApiOnly`)
        // attribute rejects a document body reassignment without warning.
        let mut parser = Parser::default().with_intrinsic_attribute_bool_silent(
            "sectids",
            false,
            ModificationContext::ApiOnly,
        );

        let doc = parser.parse(concat!("= Title\n", ":sectids:\n"));

        assert_eq!(doc.warnings().count(), 0);
        assert_eq!(parser.attribute_value("sectids"), InterpretedValue::Unset);
    }

    #[test]
    fn normally_locked_intrinsic_still_warns() {
        // Regression: a non-silent `ApiOnly` intrinsic still records
        // `AttributeValueIsLocked` when the document tries to reassign it.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        let doc = parser.parse(concat!("= Title\n", ":backend: docbook5\n"));

        assert_eq!(
            doc.warnings().next().unwrap().warning,
            WarningType::AttributeValueIsLocked("backend".to_owned())
        );
        assert_eq!(
            parser.attribute_value("backend"),
            InterpretedValue::Value("html5")
        );
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

        fn render_callout(&self, params: &crate::parser::CalloutRenderParams, dest: &mut String) {
            dest.push_str(&format!("[CALLOUT:{}]", params.number));
        }

        fn render_index_term(
            &self,
            params: &crate::parser::IndexTermRenderParams,
            dest: &mut String,
        ) {
            match params.visible_term {
                Some(term) => dest.push_str(&format!("[INDEXTERM:{term}]")),
                None => dest.push_str("[INDEXTERM]"),
            }
        }

        fn render_button(&self, text: &str, dest: &mut String) {
            dest.push_str(&format!("[BUTTON:{text}]"));
        }

        fn render_keyboard(&self, keys: &[String], dest: &mut String) {
            dest.push_str(&format!("[KBD:{}]", keys.join("+")));
        }

        fn render_menu(&self, params: &crate::parser::MenuRenderParams, dest: &mut String) {
            dest.push_str(&format!("[MENU:{}]", params.menu));
        }

        fn render_footnote(&self, params: &crate::parser::FootnoteRenderParams, dest: &mut String) {
            match params.index {
                Some(index) => dest.push_str(&format!("[FOOTNOTE:{index}]")),
                None => dest.push_str(&format!("[FOOTNOTE:{}]", params.text)),
            }
        }
    }

    #[test]
    fn with_inline_substitution_renderer() {
        let mut parser = Parser::default().with_inline_substitution_renderer(TestRenderer);

        // Parse a simple document with special characters and a footnote.
        let doc = parser.parse("Hello & goodbye < world > test footnote:[a note]");

        // The document should parse successfully.
        assert_eq!(doc.warnings().count(), 0);

        // Get the first block from the document.
        let block = doc.nested_blocks().next().unwrap();

        let Block::Simple(simple_block) = block else {
            panic!("Expected simple block, got: {block:?}");
        };

        // Our custom renderer should show [AMP], [LT], and [GT] instead of HTML
        // entities, and a resolved footnote as [FOOTNOTE:<index>].
        assert_eq!(
            simple_block.content().rendered(),
            "Hello [AMP] goodbye [LT] world [GT] test [FOOTNOTE:1]"
        );
    }

    #[test]
    fn custom_renderer_renders_unresolved_footnote() {
        let mut parser = Parser::default().with_inline_substitution_renderer(TestRenderer);

        // An unresolved footnote reference exercises the renderer's `None`
        // (no index) branch, which our custom renderer shows as
        // [FOOTNOTE:<text>].
        let doc = parser.parse("test.footnote:missing[]");

        let block = doc.nested_blocks().next().unwrap();
        let Block::Simple(simple_block) = block else {
            panic!("Expected simple block, got: {block:?}");
        };

        assert_eq!(simple_block.content().rendered(), "test.[FOOTNOTE:missing]");
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

    mod derived_doctype_attr {
        use crate::{
            document::InterpretedValue,
            parser::{AllowableValue, AttributeValue, ModificationContext, Parser},
        };

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

            // Shadow the built-in `doctype` default with an explicit unset
            // tombstone. With `doctype` no longer resolving to a `Value`, no
            // derived attribute is synthesized for any doctype.
            std::sync::Arc::make_mut(&mut parser.attribute_values).insert(
                "doctype".to_string(),
                AttributeValue {
                    allowable_value: AllowableValue::Any,
                    modification_context: ModificationContext::Anywhere,
                    silent_when_locked: false,
                    value: InterpretedValue::Unset,
                },
            );

            assert_eq!(parser.attribute_value("doctype"), InterpretedValue::Unset);
            assert_eq!(
                parser.attribute_value("backend-html5-doctype-article"),
                InterpretedValue::Unset
            );
        }

        #[test]
        fn document_header_cannot_assign_a_derived_doctype_flag() {
            // The `backend-html5-doctype-*` namespace is a read-only intrinsic,
            // so a document header assignment to it is ignored: the flag for the
            // (inactive) `book` doctype stays undefined rather than taking the
            // assigned value, so it cannot later shadow the intrinsic.
            let mut parser = Parser::default();
            let _doc = parser.parse("= Title\n:backend-html5-doctype-book: custom\n\nbody");

            assert_eq!(
                parser.attribute_value("backend-html5-doctype-book"),
                InterpretedValue::Unset
            );
        }
    }

    mod docname {
        use crate::Parser;

        #[test]
        fn none_without_primary_file_name() {
            assert_eq!(Parser::default().docname(), None);
        }

        #[test]
        fn strips_directory_and_extension() {
            assert_eq!(
                Parser::default()
                    .with_primary_file_name("mydoc.adoc")
                    .docname()
                    .as_deref(),
                Some("mydoc")
            );
            assert_eq!(
                Parser::default()
                    .with_primary_file_name("docs/guide/mydoc.adoc")
                    .docname()
                    .as_deref(),
                Some("mydoc")
            );
            // A Windows-style separator is handled too, since the primary file
            // name may be supplied on either platform.
            assert_eq!(
                Parser::default()
                    .with_primary_file_name(r"docs\guide\mydoc.adoc")
                    .docname()
                    .as_deref(),
                Some("mydoc")
            );
        }

        #[test]
        fn keeps_name_with_no_extension() {
            assert_eq!(
                Parser::default()
                    .with_primary_file_name("README")
                    .docname()
                    .as_deref(),
                Some("README")
            );
        }

        #[test]
        fn none_when_path_has_no_file_component() {
            // A primary file name that ends in a separator has an empty base
            // name, which yields no document name.
            assert_eq!(
                Parser::default()
                    .with_primary_file_name("docs/guide/")
                    .docname(),
                None
            );
        }

        #[test]
        fn leading_dot_name_is_kept_whole() {
            // A leading-dot name (e.g. `.adoc`) is treated as a dotfile with no
            // extension and kept whole, matching Ruby's
            // `File.basename(".adoc", ".*")`.
            assert_eq!(
                Parser::default()
                    .with_primary_file_name(".adoc")
                    .docname()
                    .as_deref(),
                Some(".adoc")
            );
        }
    }

    mod counter {
        use super::super::next_counter_value;
        use crate::{document::InterpretedValue, tests::prelude::*};

        #[test]
        fn next_counter_value_integer() {
            assert_eq!(next_counter_value("1"), "2");
            assert_eq!(next_counter_value("9"), "10");
            assert_eq!(next_counter_value("0"), "1");
            assert_eq!(next_counter_value("-1"), "0");
        }

        #[test]
        fn next_counter_value_non_canonical_integer_is_advanced_as_a_string() {
            // A leading zero (or sign) does not round-trip through integer
            // parsing, so it is advanced like a string instead.
            assert_eq!(next_counter_value("07"), "08");
            assert_eq!(next_counter_value("+5"), "+6");
            // A leading-zero value still carries digit-to-digit like a string.
            assert_eq!(next_counter_value("09"), "10");
            assert_eq!(next_counter_value("099"), "100");
        }

        #[test]
        fn next_counter_value_saturates_at_i64_max() {
            // A counter pinned at `i64::MAX` stays there rather than panicking
            // (debug) or wrapping (release).
            let max = i64::MAX.to_string();
            assert_eq!(next_counter_value(&max), max);
        }

        #[test]
        fn next_counter_value_characters() {
            assert_eq!(next_counter_value("a"), "b");
            assert_eq!(next_counter_value("A"), "B");
            assert_eq!(next_counter_value("z"), "aa");
            assert_eq!(next_counter_value("Z"), "AA");
            assert_eq!(next_counter_value("az"), "ba");
            assert_eq!(next_counter_value("zz"), "aaa");
            assert_eq!(next_counter_value("Zz"), "AAa");
        }

        #[test]
        fn next_counter_value_trailing_non_alphanumeric() {
            // The right-most alphanumeric is incremented; trailing punctuation is
            // left in place.
            assert_eq!(next_counter_value("a)"), "b)");
        }

        #[test]
        fn next_counter_value_no_alphanumeric() {
            // With nothing alphanumeric to carry, the final code point advances.
            assert_eq!(next_counter_value("{"), "|");
        }

        #[test]
        fn counter_defaults_to_one() {
            let p = Parser::default();
            assert_eq!(p.counter("x", None), "1");
            assert_eq!(p.counter("x", None), "2");
            assert_eq!(
                p.attribute_value("x"),
                InterpretedValue::Value("2".to_string())
            );
            assert!(p.has_attribute("x"));
            assert!(p.is_attribute_set("x"));
        }

        #[test]
        fn counter_seed_used_only_while_unset() {
            let p = Parser::default();
            assert_eq!(p.counter("c", Some("A")), "A");
            // Once set, a later seed is ignored.
            assert_eq!(p.counter("c", Some("Q")), "B");
        }

        #[test]
        fn counter_empty_seed_falls_back_to_one() {
            let p = Parser::default();
            assert_eq!(p.counter("c", Some("")), "1");
        }
    }
}
