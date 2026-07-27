use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, LazyLock},
};

use regex::Regex;

use crate::{
    Document, HasSpan,
    blocks::{SectionNumber, SectionType},
    document::{Attribute, Catalog, InterpretedValue, RefType},
    parser::{
        AllowableValue, AttributeValue, DatetimeContext, DefaultPathResolver, DocinfoFileHandler,
        HtmlSubstitutionRenderer, ImageFileHandler, IncludeFileHandler, InlineSubstitutionRenderer,
        ModificationContext, PathResolver, ReferenceTime, ResolvedAttributes, SafeMode, SourceLine,
        SourceMap, SvgFileHandler,
        built_in_attrs::{
            built_in_attr, built_in_default_values, derived_backend_value,
            is_derived_backend_value, max_attribute_value_size_default, synthesized_attr,
            user_home_default,
        },
        is_datetime_attribute,
        preprocessor::preprocess,
        safe_mode::masked_doc_path,
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

    /// The configured baseline of [`attribute_values`](Self::attribute_values):
    /// a snapshot of the API-established attributes as of the last builder
    /// call, with none of the attributes a document discovers during
    /// parsing.
    ///
    /// [`parse_deferred`](Self::parse_deferred) restores
    /// [`attribute_values`](Self::attribute_values) from this at the start of
    /// every parse, so a `Parser` reused across documents returns to its
    /// configured baseline between parses rather than leaking one document's
    /// header assignments into the next. It is refreshed (by
    /// [`capture_attribute_baseline`](Self::capture_attribute_baseline)) after
    /// each attribute-configuring builder call, so reconfiguring a `Parser`
    /// between parses is honored. Shared via [`Arc`] copy-on-write for the same
    /// reason as [`attribute_values`](Self::attribute_values), so tracking it
    /// costs only an `Arc` clone.
    ///
    /// [`attribute_values`]: Self::attribute_values
    pub(crate) baseline_attribute_values: Arc<HashMap<String, AttributeValue>>,

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
    ///
    /// Defaults to [`DefaultPathResolver`]; a host can substitute custom
    /// path/URL rewriting via
    /// [`with_path_resolver`](Self::with_path_resolver).
    pub(crate) path_resolver: Rc<dyn PathResolver>,

    /// Handler for resolving include:: directives.
    pub(crate) include_file_handler: Option<Rc<dyn IncludeFileHandler>>,

    /// Handler for resolving docinfo files. If absent, no docinfo content is
    /// resolved.
    pub(crate) docinfo_file_handler: Option<Rc<dyn DocinfoFileHandler>>,

    /// Handler for reading the contents of an SVG file requested by an inline
    /// image with the `inline` option. If absent, inline SVG images fall back
    /// to rendering their alt text.
    pub(crate) svg_file_handler: Option<Rc<dyn SvgFileHandler>>,

    /// Handler for reading the bytes of an image file that must be embedded as
    /// a `data:` URI (when the `data-uri` attribute is set below
    /// [`SafeMode::Secure`]). If absent, such images fall back to an ordinary
    /// web path.
    pub(crate) image_file_handler: Option<Rc<dyn ImageFileHandler>>,

    /// Whether referenced images are recorded in the document catalog as they
    /// are encountered (Asciidoctor's `catalog_assets` API option). When
    /// `false` (the default), the `image:`/`image::` macros do not populate
    /// [`Catalog::images`](crate::document::Catalog::images).
    pub(crate) catalog_assets: bool,

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

    /// True while a section title is being substituted, so each `footnote:[…]`
    /// macro's rendered marker is bracketed with
    /// [`FOOTNOTE_MARKER_START`](crate::content::FOOTNOTE_MARKER_START) /
    /// [`FOOTNOTE_MARKER_END`](crate::content::FOOTNOTE_MARKER_END) sentinels.
    /// The footnote is still defined and numbered in document order; the
    /// sentinels merely let the marker be excised from the section's reference
    /// text and auto-generated ID without a second substitution pass (see
    /// `SectionBlock::parse`).
    ///
    /// Wrapped in a [`Cell`] for the same reason as
    /// [`in_bibliography_list_item`](Self::in_bibliography_list_item): the
    /// substitution code paths hold only a shared reference to the parser.
    pub(crate) mark_footnote_spans: Cell<bool>,

    /// A block title carried over from a section heading to the first block
    /// parsed after it.
    ///
    /// A block title above a section heading does not title the section; it is
    /// carried over to the first block inside the section (matching
    /// Asciidoctor, where the block-attribute hash holding the title is passed
    /// through to the section body's first block). `SectionBlock::parse`
    /// stashes the rendered title here and the next block parsed claims it –
    /// which may be a nested section, re-stashing it for *its* first block, or
    /// (when the section body is empty) a sibling section reached after the
    /// stashing section ends. A block with a title of its own wins over the
    /// carried title, which is then discarded.
    ///
    /// The title is carried as an owned snapshot (this struct is lifetime-free
    /// and cannot hold the `.Title` line's source span), so a block claiming a
    /// carried title has no `title_source` – the same shape as a title
    /// supplied via a `title=` attribute. The snapshot keeps any deferred
    /// cross-references, so an embedded `<<id>>` in a carried title still
    /// resolves once the catalog is complete.
    pub(crate) pending_block_title: Option<crate::content::OwnedTitle>,

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

    /// Running state for inline `{counter:…}` / `{counter2:…}` counters whose
    /// target attribute is *locked* (API-set or a locked built-in).
    ///
    /// Such a counter must keep advancing across repeated references, but it
    /// must not overwrite the locked attribute's readable value – so its
    /// sequence is tracked here rather than in the readable
    /// [`counter_values`](Self::counter_values) overlay. This mirrors
    /// Asciidoctor's `Document#counter`, which advances `@counters` while
    /// leaving `@attributes` untouched for a locked attribute (see
    /// [`counter_impl`](Self::counter_impl)). A captioning counter is exempt:
    /// its value is committed to the readable overlay even when locked.
    ///
    /// Accepted limitation: a locked inline counter tracks its sequence here
    /// while a captioning counter tracks it in
    /// [`counter_values`](Self::counter_values), so the two sequences diverge
    /// when the *same* locked attribute is advanced both ways in one document
    /// (e.g. an API-locked `example-number` driven by both example blocks and
    /// inline `{counter:example-number}` references) – Asciidoctor keeps a
    /// single `@counters` sequence shared across both. This is left unmatched
    /// deliberately. The scenario – API-locking a `<context>-number` attribute
    /// *and* mixing caption and inline use – is pathological, and exact parity
    /// is unreachable regardless: this crate resolves inline counters during
    /// parsing, whereas Asciidoctor advances captions during parsing but inline
    /// references during conversion, so the two sequences interleave
    /// differently no matter how the state is stored. Unifying the maps would
    /// therefore add read-path complexity (the gate would have to be threaded
    /// through [`ResolvedAttributes`] too) without actually matching
    /// Asciidoctor's output here.
    pub(crate) locked_counter_values: RefCell<HashMap<String, String>>,

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
    ///
    /// The overwhelming majority of locked names are built-in attributes, whose
    /// names are already `&'static str`, so entries are stored as
    /// [`Cow<'static, str>`]: a built-in is locked with a borrowed static name
    /// (no allocation), and only a genuinely dynamic (parent-defined) name is
    /// owned. This keeps the per-cell rebuild – which runs once for every
    /// AsciiDoc cell – from allocating a `String` per built-in attribute.
    pub(crate) locked_attribute_names: HashSet<Cow<'static, str>>,

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

    /// Number of privately-owned sub-sources currently being parsed in the call
    /// stack.
    ///
    /// A Markdown-style blockquote and an AsciiDoc table cell parse their
    /// blocks from an owned source string whose byte offsets do not map to
    /// the primary document source. A footnote defined while such a
    /// sub-source is being substituted therefore cannot record a
    /// document-relative location for its cross-reference warning;
    /// `define_footnote` reads this counter to detect that case and leave
    /// the location unset (see
    /// [`Footnote::location`](crate::document::Footnote)). It is incremented
    /// and decremented around each owned sub-source parse, so it nests
    /// correctly.
    ///
    /// Unlike [`nested_document_depth`](Self::nested_document_depth), this also
    /// counts Markdown-style blockquotes (which are not nested documents), so
    /// the two cannot be merged.
    pub(crate) owned_subsource_depth: usize,

    /// Number of nested block-parsing scopes currently open on the call stack.
    ///
    /// Every level of block nesting – a delimited block's body, a section body,
    /// an AsciiDoc table cell, a nested list – is parsed by a fresh recursive
    /// descent (through [`parse_blocks_until`] or
    /// [`ListBlock::parse_inside_list`]) that consumes native stack. Without a
    /// bound, a small crafted document (strictly-increasing delimiters, or
    /// deeply-nested list markers) drives arbitrarily deep recursion and
    /// overflows the stack – an *uncatchable* abort that takes down the whole
    /// host process, not just the parse. This counter is incremented on entry
    /// to each such scope and decremented on exit; when it would exceed
    /// [`max_block_nesting`](Self::max_block_nesting) the over-nested content
    /// is truncated with a [`MaxBlockNestingExceeded`] warning instead of
    /// being descended into (see issue #885).
    ///
    /// [`parse_blocks_until`]: crate::blocks::parse_utils::parse_blocks_until
    /// [`ListBlock::parse_inside_list`]: crate::blocks::ListBlock
    /// [`MaxBlockNestingExceeded`]:
    /// crate::warnings::WarningType::MaxBlockNestingExceeded
    pub(crate) block_nesting_depth: usize,

    /// The block-nesting cap ([`max_block_nesting`](Self::max_block_nesting))
    /// resolved once per document, so the guard consulted on *every*
    /// [`parse_blocks_until`] scope entry is a plain integer compare rather
    /// than a document-attribute lookup (a `RefCell` borrow, several map
    /// lookups, a string clone, and a parse) on that hot path.
    ///
    /// `max-block-nesting` is API-only, so its value cannot change mid-parse;
    /// [`parse_deferred`](Self::parse_deferred) refreshes this from the
    /// resolved attribute at the start of each parse. It is seeded with the
    /// built-in default so a block parser invoked directly in a unit test
    /// (bypassing `parse_deferred`) still sees the right cap.
    ///
    /// [`parse_blocks_until`]: crate::blocks::parse_utils::parse_blocks_until
    pub(crate) block_nesting_limit: usize,

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

    /// Stack of source maps for the include-expanded (owned) AsciiDoc table
    /// cells currently being parsed in the call stack (innermost last).
    ///
    /// An AsciiDoc cell whose first line is an `include::` directive is parsed
    /// from a private, preprocessor-expanded copy of its content rather than
    /// from the document source. While that owned copy is being parsed a span's
    /// line number no longer indexes the document
    /// [`source_map`](Self::source_map); it indexes the owned copy instead.
    /// Each owned cell's own source map (produced by re-running the
    /// preprocessor over its content) is pushed here for the duration of
    /// its parse, so a directive buried inside the owned content can still
    /// be mapped back to the file and line it *originally* came from –
    /// needed to name that file in the "Unresolved directive" message and
    /// to report the warning's true cursor via
    /// [`Warning::origin`](crate::warnings::Warning::origin).
    ///
    /// The stack is empty while parsing the top-level document (and while
    /// parsing a *borrowed* cell, which keeps document spans). It is non-empty
    /// exactly when a span's line indexes an owned copy rather than the
    /// document, which the source-map lookup uses to decide which map to
    /// consult. It is pushed and popped around each owned-cell parse, so it
    /// nests correctly.
    pub(crate) owned_cell_source_maps: Vec<Rc<SourceMap>>,

    /// Warnings raised by an `include::` directive buried inside an owned
    /// (include-expanded) AsciiDoc table cell, each already resolved to the
    /// `(file, line)` where the directive originally appeared.
    ///
    /// Such a warning is raised deep inside the owned cell's parse, where the
    /// only spans available borrow the owned copy and cannot escape it, and no
    /// document span maps to the directive. It is therefore recorded here (in a
    /// lifetime-free, pre-resolved form) and drained once an enclosing
    /// document-level cell can anchor it to a real document span while carrying
    /// its true origin (see
    /// [`Warning::origin`](crate::warnings::Warning::origin)).
    ///
    /// Wrapped in a [`RefCell`] only for symmetry with the other deferred
    /// warning buffers; it is mutated through `&mut self`-free helpers so the
    /// owned-cell parse (which holds the parser mutably) can still record into
    /// it from within a `self_cell` construction closure.
    owned_cell_warnings: RefCell<Vec<ResolvedWarning>>,

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

    /// An optional fixed reference time that pins the clock used to compute the
    /// time-dependent document attributes (`docdate`, `doctime`, `docdatetime`,
    /// `docyear`, and their `local*` siblings), for reproducible output.
    ///
    /// When set, it supersedes both the real wall clock and the
    /// `SOURCE_DATE_EPOCH` environment variable as the value of "now" (which
    /// drives the `local*` attributes and, absent an [`input_mtime`], the
    /// `doc*` attributes too). See [`with_reference_time`] and
    /// [`resolve_datetime_attribute`].
    ///
    /// [`input_mtime`]: Self::input_mtime
    /// [`with_reference_time`]: Self::with_reference_time
    /// [`resolve_datetime_attribute`]: Self::resolve_datetime_attribute
    reference_time: Option<ReferenceTime>,

    /// An optional fixed modification time of the source document, pinning the
    /// clock that drives the `doc*` attributes (`docdate`, `doctime`,
    /// `docdatetime`, `docyear`) specifically.
    ///
    /// This mirrors Asciidoctor's `input_mtime` option: the `local*` attributes
    /// continue to track "now", while the `doc*` attributes reflect this source
    /// modification time. See [`with_input_mtime`] and
    /// [`resolve_datetime_attribute`].
    ///
    /// [`with_input_mtime`]: Self::with_input_mtime
    /// [`resolve_datetime_attribute`]: Self::resolve_datetime_attribute
    input_mtime: Option<ReferenceTime>,

    /// The reference instants used to compute the time-dependent document
    /// attributes, captured lazily the first time one of those attributes is
    /// read during a parse (and reset at the start of each parse).
    ///
    /// The time-dependent attributes are *not* materialized into
    /// [`attribute_values`](Self::attribute_values); instead they are resolved
    /// on demand from this context (see
    /// [`resolve_datetime_attribute`](Self::resolve_datetime_attribute)). A
    /// parse that never references one therefore does no clock, environment, or
    /// allocation work for them, and repeated reads within a parse observe a
    /// single consistent instant.
    ///
    /// Wrapped in a [`RefCell`] because the capture happens through the shared
    /// `&Parser` attribute readers (which the substitution code paths reach
    /// with only a shared reference).
    datetime_context: RefCell<Option<DatetimeContext>>,
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

    /// A pre-resolved originating `(file, line)` for this warning, carried
    /// through to [`Warning::origin`].
    ///
    /// This is `None` for warnings that point at real (emitted) output: their
    /// [`offset`](Self::offset)/[`len`](Self::len) span resolves the location
    /// through the document source map. It is `Some` for a preprocessor
    /// directive that produces no output of its own – a malformed or
    /// unterminated conditional directive – where there is no output span to
    /// resolve against, so the directive's own file and line are recorded here
    /// directly (the `offset`/`len` span is then only a best-effort anchor).
    ///
    /// [`Warning::origin`]: crate::warnings::Warning::origin
    pub(crate) origin: Option<SourceLine>,
}

/// A warning whose location is already resolved to an originating
/// `(file, line)`, independent of any source map.
///
/// Used for a warning raised inside an owned (include-expanded) AsciiDoc table
/// cell, whose directive never appears in the document source: it is resolved
/// against the owning cell's own source map when raised, then carried in this
/// form until an enclosing document-level cell can surface it (see
/// [`Parser::owned_cell_warnings`]).
#[derive(Clone, Debug)]
pub(crate) struct ResolvedWarning {
    /// The originating file and line where the directive appeared.
    pub(crate) origin: SourceLine,

    /// The type of warning, already carrying any owned data it needs (such as
    /// the missing include target).
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
            baseline_attribute_values: Arc::new(HashMap::new()),
            default_attribute_values: built_in_default_values(),
            renderer: Rc::new(HtmlSubstitutionRenderer {}),
            primary_file_name: None,
            path_resolver: Rc::new(DefaultPathResolver::default()),
            include_file_handler: None,
            docinfo_file_handler: None,
            svg_file_handler: None,
            image_file_handler: None,
            catalog_assets: false,
            safe: SafeMode::default(),
            catalog: RefCell::new(Catalog::new()),
            last_section_number: SectionNumber::default(),
            last_appendix_section_number: SectionNumber {
                section_type: SectionType::Appendix,
                components: vec![],
                appendix_letter: None,
            },
            sectnumlevels: 3,
            topmost_section_type: SectionType::Normal,
            parsing_bibliography_section_body: false,
            in_bibliography_list_item: Cell::new(false),
            mark_footnote_spans: Cell::new(false),
            pending_block_title: None,
            counter_values: RefCell::new(HashMap::new()),
            locked_counter_values: RefCell::new(HashMap::new()),
            locked_attribute_names: HashSet::new(),
            nested_document_depth: 0,
            owned_subsource_depth: 0,
            block_nesting_depth: 0,
            block_nesting_limit: Self::DEFAULT_MAX_BLOCK_NESTING,
            source_map: None,
            owned_cell_source_maps: vec![],
            owned_cell_warnings: RefCell::new(vec![]),
            callouts: RefCell::new(CalloutCatalog::default()),
            substitution_warnings: RefCell::new(vec![]),
            reference_time: None,
            input_mtime: None,
            datetime_context: RefCell::new(None),
        }
    }
}

impl Parser {
    /// The default value of [`max_block_nesting`](Self::max_block_nesting),
    /// mirroring the `max-block-nesting` built-in default (see
    /// [`built_in_attrs`](super::built_in_attrs)).
    pub(crate) const DEFAULT_MAX_BLOCK_NESTING: usize = 32;

    /// Creates a new [`Parser`] with the default configuration.
    ///
    /// This is equivalent to [`Parser::default()`](Self::default) and is
    /// provided for discoverability. Configure the parser by chaining the
    /// `with_*` builder methods, then call [`parse()`](Self::parse).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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
    /// [`attribute_value()`] after this call returns.
    ///
    /// # Reusing a `Parser` across documents
    ///
    /// A single `Parser` may be reused to parse many documents (in a loop, for
    /// example). Each parse begins from the `Parser`'s configured baseline –
    /// the attributes established through the builder API (e.g.
    /// [`with_intrinsic_attribute()`]) – with the document attributes
    /// discovered while parsing the *previous* document cleared.
    /// Header/body assignments (`:foo: bar`) therefore do not leak from one
    /// document into the next, so output does not depend on parse order.
    /// (Attributes discovered by a parse remain inspectable per the paragraph
    /// above only until the next parse, which clears them back to the
    /// baseline.) Reconfiguring the `Parser` with a builder method between
    /// parses updates that baseline for all subsequent parses.
    ///
    /// [`with_intrinsic_attribute()`]: Self::with_intrinsic_attribute
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
    #[must_use]
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
    #[must_use]
    pub fn parse_deferred(&mut self, source: &str) -> Document<'static> {
        // Restore the configured attribute baseline so a `Parser` reused across
        // documents does not carry one document's header/body assignments into
        // the next (which would make conditional-include, substitution, and
        // section-numbering behavior order-dependent). This must precede front-
        // matter handling and preprocessing below, both of which read document
        // attributes. See `baseline_attribute_values`.
        self.attribute_values = Arc::clone(&self.baseline_attribute_values);

        // The time-dependent document attributes (docdate, doctime, docdatetime,
        // docyear, and their local* siblings) are resolved lazily from a
        // reference instant captured the first time one is read (see
        // `resolve_datetime_attribute`). Reset that capture so each parse sees a
        // fresh "now"; a parse that never references one does no datetime work.
        *self.datetime_context.borrow_mut() = None;

        // Strip a leading UTF-8 byte-order mark (U+FEFF), which is valid but
        // non-content in a UTF-8 document. Left in place it becomes the first
        // character of the first line and defeats header, front-matter, and
        // section recognition (e.g. a leading `= ` title), silently misparsing
        // the document. Asciidoctor strips it likewise. This is independent of
        // UTF-16 transcoding, which remains out of scope; it handles only the
        // UTF-8 encoding of the mark. This must precede front-matter handling
        // and preprocessing, both of which key off the first line.
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);

        // Drop leading YAML-style front matter (and record it in the
        // `front-matter` attribute) when `skip-front-matter` is set, before the
        // source reaches the preprocessor or header parser.
        let stripped_source = self.skip_front_matter(source);
        let source = stripped_source.as_deref().unwrap_or(source);

        let (preprocessed_source, source_map, preprocessor_warnings, includes) =
            preprocess(source, self);

        // NOTE: `Document::parse` will transfer the catalog to itself at the end of the
        // parsing operation. Start each parse with a fresh catalog.
        *self.catalog.borrow_mut() = Catalog::new();

        // Seed the fresh catalog with the files the preprocessor just expanded,
        // so an inter-document cross reference to an included file can resolve
        // to an internal anchor while the document is parsed. Replaying each
        // event lets the catalog resolve a file that was included both fully and
        // partially to a full include.
        {
            let mut catalog = self.catalog.borrow_mut();
            for (key, full) in includes {
                catalog.register_include(&key, full);
            }
        }

        // Start each parse with an empty callout catalog.
        *self.callouts.borrow_mut() = CalloutCatalog::default();

        // Start each parse with no pending substitution warnings.
        self.substitution_warnings.borrow_mut().clear();

        // Reset section numbering for each new document.
        self.last_section_number = SectionNumber::default();

        // Start each parse with no block title carried over from a section
        // heading.
        self.pending_block_title = None;

        // Reset counter (and captioned-block) numbering for each new document.
        self.counter_values.borrow_mut().clear();
        self.locked_counter_values.borrow_mut().clear();

        // Start each parse at the outermost block-nesting level. The counter is
        // otherwise kept balanced by matched increment/decrement pairs, so this
        // only matters if a prior parse was abandoned mid-flight; it is reset
        // here (rather than in the recursive cell/blockquote paths) because
        // `parse_deferred` runs only for the top-level document.
        self.block_nesting_depth = 0;

        // Resolve the block-nesting cap once, now, so the guard on the hot
        // `parse_blocks_until` path is a plain integer compare. The attribute is
        // API-only and so cannot change mid-parse.
        self.block_nesting_limit = self.max_block_nesting();

        Document::parse(
            &preprocessed_source,
            source_map,
            preprocessor_warnings,
            self,
        )
    }

    /// Drops leading YAML-style front matter from `source`, mirroring
    /// Asciidoctor's `Reader#skip_front_matter!`.
    ///
    /// Front matter is a block opened by a line of exactly `---` at the very
    /// start of the document and closed by a matching `---` line. Only this
    /// `---` fence (the YAML convention) is recognized – a `+++` TOML fence is
    /// not – and the captured content is stored verbatim, never parsed. It is
    /// only removed when the `skip-front-matter` attribute is set
    /// (typically via the API); otherwise `---` retains its ordinary
    /// meaning and this is a no-op. When a well-formed block is found, its
    /// content (the lines between the delimiters, joined by LF and with the
    /// delimiters excluded) is stored in the `front-matter` document
    /// attribute, and a rewritten copy of the source is returned in which
    /// every removed line – both delimiters and the content – is replaced
    /// by a blank line. Preserving the line *count* keeps every following
    /// line at its original line number, matching Asciidoctor (whose reader
    /// advances `lineno` past the skipped block); the leading blank lines
    /// are ignored by the header parser.
    ///
    /// Returns `None` (leaving the source untouched and setting no attribute)
    /// when `skip-front-matter` is not set, when the first line is not `---`,
    /// or when no closing `---` delimiter is found.
    fn skip_front_matter(&mut self, source: &str) -> Option<String> {
        if !self.is_attribute_set("skip-front-matter") {
            return None;
        }

        // Strip a line's trailing end-of-line sequence (LF or CRLF) so the
        // delimiter comparison and the captured content match Asciidoctor's
        // chomped lines.
        fn line_content(line: &str) -> &str {
            let line = line.strip_suffix('\n').unwrap_or(line);
            line.strip_suffix('\r').unwrap_or(line)
        }

        let mut lines = source.split_inclusive('\n');

        // Front matter must open on the very first line, which must be exactly
        // `---`.
        let first = lines.next()?;
        if line_content(first) != "---" {
            return None;
        }

        // Byte offset just past the region being consumed, and the number of
        // physical lines consumed (starting with the opening delimiter).
        let mut consumed_end = first.len();
        let mut consumed_lines = 1usize;

        let mut front_matter = String::new();
        let mut closed = false;

        for line in lines {
            consumed_end += line.len();
            consumed_lines += 1;

            if line_content(line) == "---" {
                closed = true;
                break;
            }

            if !front_matter.is_empty() {
                front_matter.push('\n');
            }

            front_matter.push_str(line_content(line));
        }

        // Without a closing delimiter the block is malformed; leave the source
        // (and attributes) untouched, matching Asciidoctor.
        if !closed {
            return None;
        }

        self.set_attribute_by_value_from_header("front-matter", &front_matter);

        // Replace the consumed region with one blank line per removed physical
        // line so the remaining content keeps its original line numbers.
        let mut rewritten = String::with_capacity(source.len());
        for _ in 0..consumed_lines {
            rewritten.push('\n');
        }

        rewritten.push_str(&source[consumed_end..]);

        Some(rewritten)
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

        // An unset `relfilesuffix` reads as the *effective* value of
        // `outfilesuffix` – routed through this same reader so an
        // `outfilesuffix` counter overlay is honored too (see
        // [`tracks_outfilesuffix`](Self::tracks_outfilesuffix)).
        if self.tracks_outfilesuffix(name) {
            return self.attribute_value("outfilesuffix");
        }

        // Under `SafeMode::Server` or greater, `docdir` reads as empty and
        // `docfile` is relativized (its `docdir` prefix stripped), matching
        // Ruby Asciidoctor's document-init masking. Resolved here at read time
        // (like `relfilesuffix` above and `max-attribute-value-size`) so the
        // API-provided values are never rewritten and builder-call order does
        // not matter (see [`masked_doc_path`]).
        //
        // [`masked_doc_path`]: crate::parser::safe_mode::masked_doc_path
        if self.safe >= SafeMode::Server
            && let Some(masked) = masked_doc_path(name, |n| self.raw_set_value(n))
        {
            return masked;
        }

        // `basebackend` / `filetype` are derived on the fly from the current
        // `backend` (see [`derived_backend_value`]) rather than stored; they are
        // read-only intrinsics, so no per-parser entry ever shadows this.
        if let Some(value) = derived_backend_value(name, &self.attribute_values) {
            return value;
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

            // A time-dependent attribute is not materialized in either table; it
            // is resolved on demand from the captured reference instant.
            None => self
                .resolve_datetime_attribute(name)
                .unwrap_or(InterpretedValue::Unset),
        }
    }

    /// Returns the raw stored string value of `name` if it currently resolves
    /// to a plain [`Value`](InterpretedValue::Value), *before* any
    /// safe-mode masking is applied. Returns `None` when the attribute is unset
    /// or resolves to a non-value form.
    ///
    /// This feeds [`masked_doc_path`], which must compute the `docfile`
    /// relativization from the *original* API-provided `docdir` rather than its
    /// masked (blanked) form.
    fn raw_set_value(&self, name: &str) -> Option<String> {
        match self.effective_attribute(name)?.value {
            InterpretedValue::Value(ref v) => Some(v.clone()),
            _ => None,
        }
    }

    /// Returns the effective attribute definition for `name`: a per-parser
    /// entry (an override or an explicit [unset] tombstone) shadows the
    /// shared built-in default, which in turn shadows an on-the-fly
    /// synthesized attribute (the active `backend-html5-doctype-*` and
    /// `safe-mode-*` flags). The synthesized attributes are never
    /// materialized in either table.
    ///
    /// This is the *raw* lookup used by the attribute writers to decide whether
    /// a name is locked against modification. The attribute *readers*
    /// additionally resolve the read-only default of `relfilesuffix` (see
    /// [`tracks_outfilesuffix`](Self::tracks_outfilesuffix)).
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn effective_attribute(&self, name: &str) -> Option<&AttributeValue> {
        if let Some(av) = self.attribute_values.get(name) {
            return Some(av);
        }

        // `max-attribute-value-size` carries its `4096` default only under
        // Secure, so it is resolved as a mode-aware synthesized attribute rather
        // than a fixed built-in. It is consulted here *after* the per-parser map
        // so a caller-supplied value (always API-only, hence always in that map)
        // wins regardless of builder-call order, and a `with_safe_mode` change
        // never rewrites it.
        if name == "max-attribute-value-size" {
            return Some(max_attribute_value_size_default(
                self.safe == SafeMode::Secure,
            ));
        }

        // `user-home` is the user's home directory below `SafeMode::Server` and
        // the masking `.` at `Server`/`Secure`, so it too is resolved as a
        // mode-aware synthesized attribute. Consulted here *after* the
        // per-parser map so a caller-supplied `user-home` (always API-only,
        // hence always in that map) wins regardless of builder-call order.
        if name == "user-home" {
            return Some(user_home_default(self.safe < SafeMode::Server));
        }
        if let Some(av) = built_in_attr(name) {
            return Some(av);
        }
        synthesized_attr(name, &self.attribute_values)
    }

    /// Reports whether `name` is `relfilesuffix` in its unset state, in which
    /// case a *read* resolves it to the current value of `outfilesuffix` (the
    /// two diverge for non-HTML backends, e.g. `.xml` for DocBook – see
    /// [issue #657](https://github.com/asciidoc-rs/asciidoc-parser/issues/657)).
    ///
    /// Returns `false` once `relfilesuffix` is explicitly set or unset (an
    /// entry – a value or an [unset] tombstone – then lives in the
    /// per-parser map), and for every other name. The redirect is
    /// deliberately confined to the value *readers*: the attribute
    /// *writers* consult [`effective_attribute`](Self::effective_attribute)
    /// directly, so `relfilesuffix` stays modifiable anywhere rather than
    /// inheriting the header-only modification context of `outfilesuffix`.
    /// Callers must apply a like-named counter overlay first, so a
    /// `{counter:relfilesuffix}` still wins over the tracked default.
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn tracks_outfilesuffix(&self, name: &str) -> bool {
        name == "relfilesuffix" && !self.attribute_values.contains_key(name)
    }

    /// Returns `true` if the parser has a [document attribute] by this name.
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        let name = name.as_ref();
        if self.counter_values.borrow().contains_key(name) {
            return true;
        }
        if self.tracks_outfilesuffix(name) {
            return self.has_attribute("outfilesuffix");
        }

        // A derived `basebackend` / `filetype` is present only while `backend`
        // resolves to a non-empty value (see [`derived_backend_value`]).
        if derived_backend_value(name, &self.attribute_values).is_some() {
            return true;
        }
        self.effective_attribute(name).is_some() || self.resolve_datetime_attribute(name).is_some()
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

        if self.tracks_outfilesuffix(name) {
            return self.is_attribute_set("outfilesuffix");
        }

        // A derived `basebackend` / `filetype` holds a concrete (set) value
        // whenever it is present, i.e. while `backend` is non-empty (see
        // [`derived_backend_value`]).
        if derived_backend_value(name, &self.attribute_values).is_some() {
            return true;
        }

        self.effective_attribute(name)
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or_else(|| self.resolve_datetime_attribute(name).is_some())
    }

    /// Returns the current `leveloffset` document attribute as a signed
    /// integer.
    ///
    /// The `leveloffset` attribute shifts the effective level of every section
    /// heading in scope (see the include directive's `leveloffset` option and
    /// the `:leveloffset:` attribute entry). Relative assignments (`+N` / `-N`)
    /// are resolved to an absolute value when the attribute is set (see
    /// [`resolve_leveloffset_assignment`](Self::resolve_leveloffset_assignment)),
    /// so the stored value is always a plain integer; a non-integer or unset
    /// value yields an offset of `0`.
    pub(crate) fn level_offset(&self) -> i32 {
        match self.attribute_value("leveloffset") {
            InterpretedValue::Value(v) => v.trim().parse::<i32>().unwrap_or(0),
            _ => 0,
        }
    }

    /// Resolves the maximum block-nesting depth in effect from the
    /// `max-block-nesting` attribute (default
    /// [`DEFAULT_MAX_BLOCK_NESTING`](Self::DEFAULT_MAX_BLOCK_NESTING)). See
    /// [`block_nesting_depth`](Self::block_nesting_depth).
    ///
    /// This walks the document-attribute machinery, so it is resolved once per
    /// parse into [`block_nesting_limit`](Self::block_nesting_limit) rather
    /// than consulted on the hot [`parse_blocks_until`] path; use
    /// [`block_nesting_limit_reached`](Self::block_nesting_limit_reached)
    /// there.
    ///
    /// The attribute is API-only, so a hostile document cannot raise its own
    /// limit. The value is coerced as Ruby's `String#to_i` would (matching how
    /// `max-include-depth` is read): a non-positive result yields `0` – which
    /// permits only the outermost, document-level block scope – while a
    /// positive value too large for `usize` saturates rather than wrapping to
    /// the "disabled" sentinel.
    ///
    /// [`parse_blocks_until`]: crate::blocks::parse_utils::parse_blocks_until
    pub(crate) fn max_block_nesting(&self) -> usize {
        match self.attribute_value("max-block-nesting") {
            InterpretedValue::Value(value) => {
                let depth = super::preprocessor::ruby_to_i(&value);
                if depth <= 0 {
                    0
                } else {
                    usize::try_from(depth).unwrap_or(usize::MAX)
                }
            }

            // An explicit empty `Set` coerces to 0; an explicit unset falls back
            // to the built-in default.
            InterpretedValue::Set => 0,
            InterpretedValue::Unset => Self::DEFAULT_MAX_BLOCK_NESTING,
        }
    }

    /// Reports whether descending into another nested block scope would exceed
    /// the block-nesting limit, so the caller must stop nesting and truncate
    /// instead. See [`block_nesting_depth`](Self::block_nesting_depth).
    ///
    /// This compares against the per-parse cached
    /// [`block_nesting_limit`](Self::block_nesting_limit), so it stays cheap on
    /// the hot [`parse_blocks_until`] path.
    ///
    /// [`parse_blocks_until`]: crate::blocks::parse_utils::parse_blocks_until
    pub(crate) fn block_nesting_limit_reached(&self) -> bool {
        self.block_nesting_depth > self.block_nesting_limit
    }

    /// Records a [`MaxBlockNestingExceeded`] warning anchored at `source`,
    /// reporting the [`max_block_nesting`](Self::max_block_nesting) limit in
    /// effect.
    ///
    /// [`MaxBlockNestingExceeded`]:
    /// crate::warnings::WarningType::MaxBlockNestingExceeded
    pub(crate) fn warn_block_nesting_exceeded<'src>(
        &self,
        source: crate::Span<'src>,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        warnings.push(Warning {
            source,
            warning: WarningType::MaxBlockNestingExceeded(self.block_nesting_limit),
            origin: None,
        });
    }

    /// Returns the effective `max-attribute-value-size`: the byte limit applied
    /// to a resolved attribute-entry value, or `None` when no limit is in
    /// force.
    ///
    /// The value is coerced as Ruby's `String#to_i` would (matching
    /// Asciidoctor); a non-positive result – including an explicit unset or `0`
    /// – disables the limit. The `4096` default only exists under
    /// `SafeMode::Secure` (see
    /// [`apply_safe_mode_attributes`](Self::apply_safe_mode_attributes)), so in
    /// a relaxed safe mode this resolves to `None` unless the caller sets an
    /// explicit positive value.
    fn max_attribute_value_size(&self) -> Option<usize> {
        match self.attribute_value("max-attribute-value-size") {
            InterpretedValue::Value(value) => {
                let size = super::preprocessor::ruby_to_i(&value);
                (size > 0).then(|| usize::try_from(size).unwrap_or(usize::MAX))
            }
            _ => None,
        }
    }

    /// Applies the [`max-attribute-value-size`](Self::max_attribute_value_size)
    /// limit to a freshly resolved attribute-entry `value`, truncating it (on a
    /// character boundary, so a multibyte character is never split) when it
    /// exceeds the limit. Values that already fit, and non-`Value` variants,
    /// are returned unchanged.
    fn limit_attribute_value_size(&self, value: InterpretedValue) -> InterpretedValue {
        let Some(max) = self.max_attribute_value_size() else {
            return value;
        };

        let InterpretedValue::Value(text) = value else {
            return value;
        };

        if text.len() <= max {
            return InterpretedValue::Value(text);
        }

        // Back up to the nearest character boundary at or below `max` so a
        // multibyte character straddling the limit is dropped whole rather than
        // split (which would otherwise yield invalid UTF-8).
        let mut end = max;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }

        let mut text = text;
        text.truncate(end);
        InterpretedValue::Value(text)
    }

    /// Resolves a `leveloffset` assignment value, converting a relative form
    /// (`+N` / `-N`) into the absolute value it produces given the
    /// `leveloffset` currently in effect. Absolute values, and values that
    /// aren't a signed integer, are returned unchanged.
    ///
    /// This mirrors Asciidoctor, where a relative `leveloffset` accumulates on
    /// top of the offset already in effect. That accumulation is what lets the
    /// offsets of nested includes compose: each `include::[leveloffset=+1]`
    /// (and its `:leveloffset: +1` wrapper) shifts headings one level further
    /// down relative to wherever the surrounding content already sits.
    fn resolve_leveloffset_assignment(&self, value: InterpretedValue) -> InterpretedValue {
        let InterpretedValue::Value(ref v) = value else {
            return value;
        };

        // Only a leading `+`/`-` marks a relative assignment; anything else
        // (an absolute value, or a non-numeric value) is stored unchanged.
        let trimmed = v.trim();
        if !trimmed.starts_with(['+', '-']) {
            return value;
        }

        // Parse the whole signed value as `i64` so the extreme relative delta
        // `-2147483648` (whose magnitude exceeds `i32::MAX`) is still read as
        // itself rather than failing and being stored as an absolute value.
        match trimmed.parse::<i64>() {
            // The running offset is a valid `i32`, so widening it to `i64`
            // makes the accumulation itself infallible; `saturating_add` then
            // guards the (already absurd) case of a delta near `i64::MIN/MAX`,
            // and the result is clamped back into the `i32` the attribute
            // stores. This keeps a pathological offset from overflowing –
            // which would panic in debug builds and wrap in release builds –
            // rather than imposing a real bound the syntax does not otherwise
            // impose.
            Ok(delta) => InterpretedValue::Value(
                (self.level_offset() as i64)
                    .saturating_add(delta)
                    .clamp(i32::MIN as i64, i32::MAX as i64)
                    .to_string(),
            ),
            Err(_) => value,
        }
    }

    /// Resolves a `leveloffset` assignment (see
    /// [`resolve_leveloffset_assignment`](Self::resolve_leveloffset_assignment))
    /// and, if the resulting absolute offset is so large or small that *every*
    /// heading would be shifted outside the supported 1..=5 section-level
    /// range, records a [`LeveloffsetExcludesAllHeadingLevels`] warning
    /// against `span`.
    ///
    /// [`LeveloffsetExcludesAllHeadingLevels`]:
    /// crate::warnings::WarningType::LeveloffsetExcludesAllHeadingLevels
    fn resolve_leveloffset_and_warn<'src>(
        &self,
        value: InterpretedValue,
        span: crate::Span<'src>,
        warnings: &mut Vec<Warning<'src>>,
    ) -> InterpretedValue {
        let value = self.resolve_leveloffset_assignment(value);

        if let InterpretedValue::Value(ref v) = value
            && let Ok(offset) = v.trim().parse::<i32>()
            && !leveloffset_admits_any_heading(offset)
        {
            warnings.push(Warning {
                source: span,
                warning: WarningType::LeveloffsetExcludesAllHeadingLevels(offset),
                origin: None,
            });
        }

        value
    }

    /// Captures the parser's fully-resolved document-attribute state so it can
    /// outlive the parser – for example, retained on a [`Document`] to answer
    /// [`attribute_value`]/[`has_attribute`]/[`is_attribute_set`] without a
    /// parser in hand (the embed path a renderer uses for `convert_document`).
    ///
    /// This shares the parser's attribute tables by [`Arc`] rather than copying
    /// them, so it is cheap to take on every parse (the large built-in table is
    /// never deep-cloned). The time-dependent attributes are not materialized;
    /// the snapshot carries the reference-time configuration so it can resolve
    /// them on demand exactly as the parser does. See [`ResolvedAttributes`].
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
            self.safe,
            self.reference_time.clone(),
            self.input_mtime.clone(),
        )
    }

    /// Resolves whether a document title should be displayed, from the
    /// `showtitle`/`notitle` attribute pair (which are complements).
    ///
    /// `showtitle` takes precedence: if present, the title shows precisely when
    /// it is set. Otherwise `notitle`, if present, hides the title when set.
    /// When neither attribute is present, `default_shown` decides – a
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

    /// Applies the `notitle` ⇔ `showtitle` inverse-toggle linkage
    /// (Asciidoctor asciidoctor/asciidoctor#3804).
    ///
    /// `notitle` and `showtitle` are two spellings of a single "show the
    /// document title" switch, wired as opposites: assigning either attribute
    /// updates the other so the resolved document reflects one consistent
    /// toggle. This yields last-assignment-wins semantics for free, since each
    /// assignment rewrites the partner left by the previous one.
    ///
    /// `attr_name` is the attribute just assigned and `value` its stored value;
    /// the call is a no-op for any other name. Turning the toggle *off* (an
    /// explicit [unset], e.g. `:!notitle:`) turns the partner *on* – it is
    /// stored [set] with the same `modification_context` and
    /// `silent_when_locked` flag as the triggering assignment. Turning the
    /// toggle *on* (an empty `Set` or an explicit value, e.g. `:notitle:`)
    /// *removes* the partner entirely.
    ///
    /// The partner is removed – rather than left as an explicit unset
    /// tombstone – to mirror Asciidoctor's attribute-hash semantics, where an
    /// "off" attribute is simply absent. That keeps every observer consistent:
    /// `has_attribute`, `ifdef`/`ifndef`, and `{partner}` reference
    /// substitution all see the same absence Asciidoctor does (so, e.g., a
    /// `{showtitle}` reference stays literal after `:notitle:` rather than
    /// silently resolving to an empty string).
    ///
    /// [set]: https://docs.asciidoctor.org/asciidoc/latest/attributes/set-attributes/
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    fn apply_title_visibility_linkage(
        &mut self,
        attr_name: &str,
        value: &InterpretedValue,
        modification_context: ModificationContext,
        silent_when_locked: bool,
    ) {
        let partner = match attr_name {
            "notitle" => "showtitle",
            "showtitle" => "notitle",
            _ => return,
        };

        // Either way the partner supersedes (and resets) any counter of the
        // same name, mirroring a direct assignment.
        self.counter_values.borrow_mut().remove(partner);

        if let InterpretedValue::Unset = value {
            // The toggle is off, so the partner turns on.
            Arc::make_mut(&mut self.attribute_values).insert(
                partner.to_string(),
                AttributeValue {
                    allowable_value: AllowableValue::Any,
                    modification_context,
                    silent_when_locked,
                    value: InterpretedValue::Set,
                },
            );
        } else if self.attribute_values.contains_key(partner) {
            // The toggle is on, so the partner turns off – and, matching
            // Asciidoctor, "off" means absent. (Guarded so the common case of
            // no prior partner entry does not clone the shared map.)
            Arc::make_mut(&mut self.attribute_values).remove(partner);
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

    /// Records the current [`attribute_values`](Self::attribute_values) as the
    /// configured baseline restored at the start of each parse.
    ///
    /// Called at the end of every attribute-configuring builder method, once
    /// all of that call's effects (including any
    /// [title-visibility](Self::apply_title_visibility_linkage) partner or
    /// safe-mode attributes) have been applied. The snapshot is an [`Arc`]
    /// clone, so it captures exactly the post-call state and stays cheap; see
    /// [`baseline_attribute_values`](Self::baseline_attribute_values).
    fn capture_attribute_baseline(&mut self) {
        self.baseline_attribute_values = Arc::clone(&self.attribute_values);
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
    #[must_use]
    pub fn with_intrinsic_attribute<N: AsRef<str>, V: AsRef<str>>(
        mut self,
        name: N,
        value: V,
        modification_context: ModificationContext,
    ) -> Self {
        let name = alias_attr_name(name.as_ref().to_lowercase());
        let value = InterpretedValue::Value(value.as_ref().to_string());
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: false,
            value: value.clone(),
        };

        Arc::make_mut(&mut self.attribute_values).insert(name.clone(), attribute_value);

        self.apply_title_visibility_linkage(&name, &value, modification_context, false);

        self.capture_attribute_baseline();

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
    #[must_use]
    pub fn with_intrinsic_attribute_silent<N: AsRef<str>, V: AsRef<str>>(
        mut self,
        name: N,
        value: V,
        modification_context: ModificationContext,
    ) -> Self {
        let name = alias_attr_name(name.as_ref().to_lowercase());
        let value = InterpretedValue::Value(value.as_ref().to_string());
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: true,
            value: value.clone(),
        };

        Arc::make_mut(&mut self.attribute_values).insert(name.clone(), attribute_value);

        self.apply_title_visibility_linkage(&name, &value, modification_context, true);

        self.capture_attribute_baseline();

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

    /// Records a referenced image in the document catalog when
    /// [`catalog_assets`](Self::with_catalog_assets) is enabled. A no-op
    /// otherwise.
    ///
    /// `target` is the (already attribute-substituted) image target as written
    /// in the macro; `imagesdir` is the value of the document `imagesdir`
    /// attribute at the point of reference, or `None` when it is unset.
    ///
    /// Takes `&self` so it can be called from the macros substitution step,
    /// which only holds a shared reference to the parser.
    pub(crate) fn register_image(&self, target: String, imagesdir: Option<String>) {
        if self.catalog_assets {
            self.catalog.borrow_mut().register_image(target, imagesdir);
        }
    }

    /// Records a referenced link target in the document catalog when
    /// [`catalog_assets`](Self::with_catalog_assets) is enabled. A no-op
    /// otherwise.
    ///
    /// `target` is the final link target as it appears in the rendered `href`
    /// (e.g. `https://example.org`, `mailto:fred@example.com`).
    ///
    /// Takes `&self` so it can be called from the macros substitution step,
    /// which only holds a shared reference to the parser.
    pub(crate) fn register_link(&self, target: String) {
        if self.catalog_assets {
            self.catalog.borrow_mut().register_link(target);
        }
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
    /// `source` is the span of the content the defining `footnote:[…]` macro
    /// was written in; its offset into the document source is recorded so a
    /// cross-reference warning can be anchored at the footnote rather than at
    /// the whole document. When the footnote is defined while substituting a
    /// privately-owned sub-source (a Markdown-style blockquote or an AsciiDoc
    /// table cell – see
    /// [`owned_subsource_depth`](Self::owned_subsource_depth)), that offset
    /// does not map to the document, so no location is recorded and
    /// resolution falls back to the whole-document span.
    ///
    /// Takes `&self` so it can be called from the macros substitution step.
    pub(crate) fn define_footnote(
        &self,
        id: Option<&str>,
        text: String,
        xrefs: Vec<crate::content::XrefSegment>,
        source: crate::Span<'_>,
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

        // Record the defining occurrence's location only when it is locatable in
        // the document source. A footnote defined inside an owned sub-source
        // indexes that private source, whose offset would misplace the warning,
        // so it is left unrecorded (resolution then falls back to the
        // whole-document span).
        let location = if self.owned_subsource_depth == 0 {
            Some((source.byte_offset(), source.data().len()))
        } else {
            None
        };

        self.catalog
            .borrow_mut()
            .register_footnote(crate::document::Footnote {
                index: index.clone(),
                id: id.map(|s| s.to_owned()),
                text,
                deferred,
                location,
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
                origin: None,
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

    /// Removes and returns any substitution warnings recorded since the buffer
    /// held `len` entries.
    pub(crate) fn drain_substitution_warnings_since(&self, len: usize) -> Vec<DeferredWarning> {
        self.substitution_warnings.borrow_mut().split_off(len)
    }

    /// Takes the substitution warnings recorded during parsing, leaving the
    /// buffer empty.
    pub(crate) fn take_substitution_warnings(&self) -> Vec<DeferredWarning> {
        std::mem::take(&mut *self.substitution_warnings.borrow_mut())
    }

    /// Returns `true` while the parser is parsing the content of an owned
    /// (include-expanded) AsciiDoc table cell, i.e. when a span's line indexes
    /// an owned copy rather than the document source.
    pub(crate) fn is_in_owned_cell_source(&self) -> bool {
        !self.owned_cell_source_maps.is_empty()
    }

    /// Pushes an owned cell's source map for the duration of its parse. Paired
    /// with [`pop_owned_cell_source_map`](Self::pop_owned_cell_source_map).
    pub(crate) fn push_owned_cell_source_map(&mut self, source_map: Rc<SourceMap>) {
        self.owned_cell_source_maps.push(source_map);
    }

    /// Pops the source map pushed by the matching
    /// [`push_owned_cell_source_map`](Self::push_owned_cell_source_map).
    pub(crate) fn pop_owned_cell_source_map(&mut self) {
        self.owned_cell_source_maps.pop();
    }

    /// Resolves a line number in the innermost owned cell's source back to the
    /// file and line it originally came from, using that cell's source map.
    ///
    /// Returns `None` when not inside an owned cell source.
    pub(crate) fn owned_cell_original_file_and_line(&self, line: usize) -> Option<SourceLine> {
        self.owned_cell_source_maps
            .last()
            .and_then(|sm| sm.original_file_and_line(line))
    }

    /// Records a warning raised by a directive at `line` in the innermost owned
    /// cell's source, resolving `line` to the file and line it originally came
    /// from so the warning can be surfaced later with a real cursor (see
    /// [`take_owned_cell_warnings`]).
    ///
    /// A no-op when not inside an owned cell source (the line does not resolve
    /// to an owned origin) – the caller only reaches this from an owned-cell
    /// parse, but the guard keeps a stray call from recording an unanchorable
    /// warning.
    ///
    /// Takes `&self`: an owned-cell parse holds the parser mutably behind a
    /// `self_cell` construction closure, so recording goes through interior
    /// mutability.
    ///
    /// [`take_owned_cell_warnings`]: Self::take_owned_cell_warnings
    pub(crate) fn record_owned_cell_warning(
        &self,
        line: usize,
        warning: WarningType,
        origin_override: Option<SourceLine>,
    ) {
        // A no-output directive that originated in a file the cell *included*
        // carries a true `(file, line)` origin already; prefer it. Otherwise
        // resolve the cell's own directive line through the enclosing owned
        // cell's source map.
        let origin = origin_override.or_else(|| self.owned_cell_original_file_and_line(line));
        if let Some(origin) = origin {
            self.owned_cell_warnings
                .borrow_mut()
                .push(ResolvedWarning { origin, warning });
        }
    }

    /// Takes the owned-cell warnings recorded during parsing, leaving the
    /// buffer empty.
    pub(crate) fn take_owned_cell_warnings(&self) -> Vec<ResolvedWarning> {
        std::mem::take(&mut *self.owned_cell_warnings.borrow_mut())
    }

    /// Generate a unique ID derived from `base_id` and register it in the
    /// document catalog, returning the ID that was assigned.
    pub(crate) fn generate_and_register_unique_id(
        &self,
        base_id: &str,
        reftext: Option<&str>,
        ref_type: RefType,
    ) -> String {
        // A synthetic ID that collides with an existing one is enumerated using
        // the `idseparator` (e.g. `_section_one`, `_section_one_2`), matching
        // Ruby Asciidoctor – not a hardcoded hyphen. Mirrors the separator
        // resolution in `generate_section_id`.
        let separator = self
            .attribute_value("idseparator")
            .as_maybe_str()
            .unwrap_or_default()
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();

        self.catalog
            .borrow_mut()
            .generate_and_register_unique_id(base_id, reftext, ref_type, &separator)
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
    #[must_use]
    pub fn with_intrinsic_attribute_bool<N: AsRef<str>>(
        mut self,
        name: N,
        value: bool,
        modification_context: ModificationContext,
    ) -> Self {
        let name = alias_attr_name(name.as_ref().to_lowercase());
        let value = if value {
            InterpretedValue::Set
        } else {
            InterpretedValue::Unset
        };
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: false,
            value: value.clone(),
        };

        Arc::make_mut(&mut self.attribute_values).insert(name.clone(), attribute_value);

        self.apply_title_visibility_linkage(&name, &value, modification_context, false);

        self.capture_attribute_baseline();

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
    #[must_use]
    pub fn with_intrinsic_attribute_bool_silent<N: AsRef<str>>(
        mut self,
        name: N,
        value: bool,
        modification_context: ModificationContext,
    ) -> Self {
        let name = alias_attr_name(name.as_ref().to_lowercase());
        let value = if value {
            InterpretedValue::Set
        } else {
            InterpretedValue::Unset
        };
        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context,
            silent_when_locked: true,
            value: value.clone(),
        };

        Arc::make_mut(&mut self.attribute_values).insert(name.clone(), attribute_value);

        self.apply_title_visibility_linkage(&name, &value, modification_context, true);

        self.capture_attribute_baseline();

        self
    }

    /// Pins the reference time (the value of "now") used to compute the
    /// time-dependent document attributes, for reproducible output.
    ///
    /// AsciiDoc derives `localdate`, `localtime`, `localdatetime`, and
    /// `localyear` from the current wall-clock time, and `docdate`, `doctime`,
    /// `docdatetime`, and `docyear` from the source file's modification time
    /// (falling back to "now" when no modification time is known). Because
    /// those values change from run to run, any output that embeds them is
    /// not reproducible. Supplying a [`ReferenceTime`] pins "now" to a
    /// fixed instant so the computed attributes are stable.
    ///
    /// This is the API counterpart of the `SOURCE_DATE_EPOCH` environment
    /// variable; a value set here takes precedence over that variable. To pin
    /// only the source-modification time that drives the `doc*` attributes
    /// (leaving `local*` on the real clock), use [`with_input_mtime`] instead;
    /// an [`with_input_mtime`] value takes precedence over this one for the
    /// `doc*` attributes.
    ///
    /// A value set via the document header or body (e.g. an explicit
    /// `:docdate:`) still wins over the computed default.
    ///
    /// [`with_input_mtime`]: Self::with_input_mtime
    #[must_use]
    pub fn with_reference_time(mut self, reference_time: ReferenceTime) -> Self {
        self.reference_time = Some(reference_time);
        self
    }

    /// Pins the modification time of the source document, which drives the
    /// `docdate`, `doctime`, `docdatetime`, and `docyear` attributes.
    ///
    /// This mirrors Asciidoctor's `input_mtime` option: the `local*` attributes
    /// continue to reflect "now" (the real clock, a [`with_reference_time`]
    /// value, or `SOURCE_DATE_EPOCH`), while the `doc*` attributes reflect the
    /// supplied source modification time. A value set here takes precedence
    /// over a [`with_reference_time`] value for the `doc*` attributes.
    ///
    /// A value set via the document header or body (e.g. an explicit
    /// `:docdate:`) still wins over the computed default.
    ///
    /// [`with_reference_time`]: Self::with_reference_time
    #[must_use]
    pub fn with_input_mtime(mut self, input_mtime: ReferenceTime) -> Self {
        self.input_mtime = Some(input_mtime);
        self
    }

    /// Resolves a time-dependent document attribute (`docdate`, `doctime`,
    /// `docdatetime`, `docyear`, or a `local*` sibling) on demand, returning
    /// `None` for any other name (or when the attribute resolves to no value,
    /// as `docyear` / `localyear` do for an explicit date without a `YYYY-`
    /// prefix).
    ///
    /// The reference instant is captured lazily on the first such read of a
    /// parse and cached (see [`datetime_context`](Self::datetime_context)), so
    /// a parse that never references a time-dependent attribute does no
    /// clock, environment, or allocation work, and repeated reads observe
    /// one consistent instant. An explicit value assigned via the API,
    /// header, or body always wins; the derived `*year` / `*datetime` are
    /// computed from whichever value each sibling resolves to. See
    /// [`DatetimeContext`].
    ///
    /// Takes `&self` so it can be called from the shared-reference attribute
    /// readers (which the substitution code paths reach with only a `&Parser`);
    /// the lazy capture goes through the [`RefCell`].
    fn resolve_datetime_attribute(&self, name: &str) -> Option<InterpretedValue> {
        if !is_datetime_attribute(name) {
            return None;
        }

        let context = {
            let mut slot = self.datetime_context.borrow_mut();
            slot.get_or_insert_with(|| {
                DatetimeContext::capture(self.reference_time.as_ref(), self.input_mtime.as_ref())
            })
            .clone()
        };

        context
            .resolve(name, |sibling| self.stored_datetime_override(sibling))
            .map(InterpretedValue::Value)
    }

    /// Returns the *explicitly-set* value of `name` from the per-parser
    /// attribute map, as an owned string (a value-less "set" reads as an empty
    /// string), or `None` when it has no such entry.
    ///
    /// This reads only the stored overrides – never the on-the-fly datetime
    /// resolution – so it can supply the explicit sibling values
    /// [`resolve_datetime_attribute`](Self::resolve_datetime_attribute) needs
    /// without recursing. It mirrors the Ruby truthiness the datetime
    /// computation relies on (`attrs['docdate']`), where any present value –
    /// including an empty string – counts as explicitly supplied.
    fn stored_datetime_override(&self, name: &str) -> Option<String> {
        self.attribute_values
            .get(name)
            .and_then(|av| match &av.value {
                InterpretedValue::Value(value) => Some(value.clone()),
                InterpretedValue::Set => Some(String::new()),
                InterpretedValue::Unset => None,
            })
    }

    /// Replace the default [`InlineSubstitutionRenderer`] for this parser.
    ///
    /// The default implementation of [`InlineSubstitutionRenderer`] that is
    /// provided is suitable for HTML5 rendering. If you are targeting a
    /// different back-end rendering, you will need to provide your own
    /// implementation and set it using this call before parsing.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn with_svg_file_handler<SFH: SvgFileHandler + 'static>(mut self, handler: SFH) -> Self {
        self.svg_file_handler = Some(Rc::new(handler));
        self
    }

    /// Sets the [`ImageFileHandler`] for this parser.
    ///
    /// The image file handler is responsible for providing the raw bytes of an
    /// image that must be embedded as a `data:` URI – i.e. when the `data-uri`
    /// document attribute is set and the safe mode is below
    /// [`SafeMode::Secure`]. If no handler is provided (or it cannot find the
    /// file), such images fall back to an ordinary web path, exactly as if
    /// `data-uri` were not set.
    ///
    /// [`ImageFileHandler`]: crate::parser::ImageFileHandler
    #[must_use]
    pub fn with_image_file_handler<IFH: ImageFileHandler + 'static>(
        mut self,
        handler: IFH,
    ) -> Self {
        self.image_file_handler = Some(Rc::new(handler));
        self
    }

    /// Sets the [`PathResolver`] for this parser.
    ///
    /// The path resolver turns an asset target (an image `src`, a stylesheet
    /// href, and so on) into the clean, resolved path that appears in the
    /// rendered output. The default is [`DefaultPathResolver`], which mirrors
    /// Ruby Asciidoctor. A host that needs custom path or URL rewriting – a
    /// virtual filesystem, a content root, URL slugs – can supply its own
    /// implementation here.
    ///
    /// [`PathResolver`]: crate::parser::PathResolver
    /// [`DefaultPathResolver`]: crate::parser::DefaultPathResolver
    #[must_use]
    pub fn with_path_resolver<PR: PathResolver + 'static>(mut self, path_resolver: PR) -> Self {
        self.path_resolver = Rc::new(path_resolver);
        self
    }

    /// Enables or disables cataloging of referenced image assets.
    ///
    /// When enabled (Asciidoctor's `catalog_assets` API option), each image
    /// referenced by an `image:`/`image::` macro is recorded in the document
    /// catalog and can be retrieved afterward via
    /// [`Catalog::images`](crate::document::Catalog::images). The default is
    /// disabled, in which case no image references are recorded.
    #[must_use]
    pub fn with_catalog_assets(mut self, catalog_assets: bool) -> Self {
        self.catalog_assets = catalog_assets;
        self
    }

    /// Sets the [`SafeMode`] under which the document is parsed and rendered.
    ///
    /// The default is [`SafeMode::Secure`], the most conservative setting.
    /// Relaxing the safe mode enables security-sensitive rendering behavior,
    /// such as rendering an interactive SVG image as an `<object>` element.
    ///
    /// [`SafeMode`]: crate::SafeMode
    #[must_use]
    pub fn with_safe_mode(mut self, safe: SafeMode) -> Self {
        self.safe = safe;
        self.apply_safe_mode_attributes();

        self.capture_attribute_baseline();

        self
    }

    /// Overrides the `safe-mode-*` family of [intrinsic attributes] from the
    /// current safe mode.
    ///
    /// These attributes let a document (or a downstream converter) inspect the
    /// security mode under which it is being processed:
    ///
    /// * `safe-mode-level` – the numeric level (`0`, `1`, `10`, or `20`).
    /// * `safe-mode-name` – the lowercase mode name (`unsafe`, `safe`,
    ///   `server`, or `secure`).
    /// * `safe-mode-<name>` – a single flag attribute (set to an empty value)
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

        // NOTE: `max-attribute-value-size` is deliberately *not* touched here.
        // Its Secure-only default is resolved as a mode-aware synthesized
        // attribute in [`effective_attribute`](Self::effective_attribute), so a
        // caller's explicit limit (which lives in `attribute_values`) is never
        // clobbered by a safe-mode change, whatever the builder-call order.
    }

    /// Returns the [`SafeMode`] under which this parser operates.
    ///
    /// [`SafeMode`]: crate::SafeMode
    pub fn safe_mode(&self) -> SafeMode {
        self.safe
    }

    /// Returns the [`ImageFileHandler`] registered on this parser, if any.
    ///
    /// A custom [`InlineSubstitutionRenderer`] that resolves image URIs itself
    /// (rather than inheriting [`image_uri`]'s default `data-uri` embedding)
    /// can use this to read an image's bytes through the same handler the
    /// built-in HTML renderer uses. Returns `None` when no handler was
    /// registered via [`with_image_file_handler`], in which case there is
    /// no way to embed images and a web path should be used instead.
    ///
    /// [`ImageFileHandler`]: crate::parser::ImageFileHandler
    /// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
    /// [`image_uri`]: crate::parser::InlineSubstitutionRenderer::image_uri
    /// [`with_image_file_handler`]: Self::with_image_file_handler
    pub fn image_file_handler(&self) -> Option<&dyn ImageFileHandler> {
        self.image_file_handler.as_deref()
    }

    /// Returns the [`SvgFileHandler`] registered on this parser, if any.
    ///
    /// A custom [`InlineSubstitutionRenderer`] that renders inline SVG images
    /// itself (rather than inheriting [`render_image`]'s `opts=inline`
    /// handling) can use this to read an SVG's contents through the same
    /// handler the built-in HTML renderer uses. Returns `None` when no
    /// handler was registered via [`with_svg_file_handler`], in which case
    /// inline SVG contents are unavailable and the alt text should be used
    /// instead.
    ///
    /// [`SvgFileHandler`]: crate::parser::SvgFileHandler
    /// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
    /// [`render_image`]: crate::parser::InlineSubstitutionRenderer::render_image
    /// [`with_svg_file_handler`]: Self::with_svg_file_handler
    pub fn svg_file_handler(&self) -> Option<&dyn SvgFileHandler> {
        self.svg_file_handler.as_deref()
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

    /// Returns `true` if the AsciiDoc file named by `key` (an inter-document
    /// xref path – relative to this document, AsciiDoc extension removed) was
    /// included into this document *in full* by the preprocessor.
    ///
    /// A cross reference to such a file collapses to a same-document reference,
    /// since the file's anchors are now part of this document. Takes `&self` so
    /// it can be called from an inline-substitution
    /// [`Replacer`](regex::Replacer) that holds only a shared reference to
    /// the parser. See
    /// [`Catalog::include_is_full`](crate::document::Catalog::include_is_full).
    pub(crate) fn catalog_include_is_full(&self, key: &str) -> bool {
        self.catalog.borrow().include_is_full(key)
    }

    /// Records an included AsciiDoc file in the document catalog's include
    /// registry, mid-parse.
    ///
    /// `Parser::parse_deferred` seeds the registry with the outermost
    /// document's own includes before parsing begins; this entry point is for
    /// an include performed while a nested scope with a shared catalog – an
    /// AsciiDoc table cell – is parsed. Takes `&self` for the same reason as
    /// [`catalog_include_is_full`](Self::catalog_include_is_full). See
    /// [`Catalog::register_include`](crate::document::Catalog::register_include).
    pub(crate) fn register_include(&self, key: &str, full: bool) {
        self.catalog.borrow_mut().register_include(key, full);
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

        // The derived backend-family namespace is a read-only synthesized
        // intrinsic; a document must not write any of it (see
        // [`is_reserved_derived_attr`]).
        if is_reserved_derived_attr(&attr_name) {
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
                    origin: None,
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

        // A relative `leveloffset` (`+N` / `-N`) accumulates on top of the
        // offset already in effect; resolve it to an absolute value so the
        // stored attribute is always a plain integer, and warn if the result is
        // so extreme that no heading could ever land in the valid level range.
        if attr_name == "leveloffset" {
            value = self.resolve_leveloffset_and_warn(value, attr.span(), warnings);
        }

        // Cap the resolved value at `max-attribute-value-size` bytes (a no-op
        // unless that limit is in force – by default, only under Secure).
        value = self.limit_attribute_value_size(value);

        // `notitle` and `showtitle` are inverse spellings of one title-
        // visibility toggle; keep the partner in sync (see
        // [`apply_title_visibility_linkage`](Self::apply_title_visibility_linkage)).
        self.apply_title_visibility_linkage(
            &attr_name,
            &value,
            ModificationContext::Anywhere,
            false,
        );

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
    /// The derivation is skipped – so an explicit `iconsdir` wins – when either
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

        // The derived backend-family namespace is a read-only synthesized
        // intrinsic; a document must not write any of it (see
        // [`is_reserved_derived_attr`]).
        if is_reserved_derived_attr(&attr_name) {
            return;
        }

        // An attribute inherited from the parent document of an AsciiDoc table
        // cell is locked for the duration of that cell: a body assignment to it
        // is silently ignored (no warning), matching Asciidoctor.
        if self.locked_attribute_names.contains(attr_name.as_str()) {
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
                    origin: None,
                });
            }
            return;
        }

        let mut value = attr.value().clone();

        // A relative `leveloffset` (`+N` / `-N`) accumulates on top of the
        // offset already in effect; resolve it to an absolute value so the
        // stored attribute is always a plain integer, and warn if the result is
        // so extreme that no heading could ever land in the valid level range.
        if attr_name == "leveloffset" {
            value = self.resolve_leveloffset_and_warn(value, attr.span(), warnings);
        }

        // Cap the resolved value at `max-attribute-value-size` bytes (a no-op
        // unless that limit is in force – by default, only under Secure).
        value = self.limit_attribute_value_size(value);

        // `notitle` and `showtitle` are inverse spellings of one title-
        // visibility toggle; keep the partner in sync (see
        // [`apply_title_visibility_linkage`](Self::apply_title_visibility_linkage)).
        self.apply_title_visibility_linkage(
            &attr_name,
            &value,
            ModificationContext::Anywhere,
            false,
        );

        let attribute_value = AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            silent_when_locked: false,
            value,
        };

        // An explicit assignment supersedes (and resets) any counter of the same
        // name. This is what lets `:!name:` reset a counter.
        self.counter_values.borrow_mut().remove(&attr_name);

        // The derived `backend-html5-doctype-*` attribute tracks `doctype`
        // automatically (it is synthesized on lookup), so no refresh is needed.
        Arc::make_mut(&mut self.attribute_values).insert(attr_name, attribute_value);
    }

    /// Unlocks each *flexible* document attribute ([`FLEXIBLE_ATTRIBUTES`],
    /// currently just `sectnums`) that was supplied *set* through the API, so a
    /// later document-body assignment may still toggle it.
    ///
    /// Mirrors the flexible-attribute unfreeze at the end of Asciidoctor's
    /// `save_attributes` (run by `finalize_header`, after the header is parsed
    /// and before the body): an API attribute override whose value is *truthy*
    /// is dropped from the locked overrides, while one whose value is an
    /// [unset] (from `numbered!` / `sectnums!`) is kept locked. That asymmetry
    /// is exactly what lets an API-*enabled* `numbered` still be toggled off by
    /// a body `:numbered!:`, while an API-*disabled* `numbered!` stays
    /// permanently unnumbered even across a later `:numbered:`.
    ///
    /// Called once, for the top-level document only – Asciidoctor guards the
    /// unfreeze with `unless @parent_document`, and an AsciiDoc table cell
    /// never reaches this path. This must *not* recapture the attribute
    /// baseline: the unlock is a per-parse effect, so a `Parser` reused across
    /// documents re-derives it from the still-locked API override each time.
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn unlock_flexible_attributes(&mut self) {
        for name in FLEXIBLE_ATTRIBUTES {
            // Only an API-set (`ApiOnly`, i.e. locked) override with a value
            // that is not an explicit unset is unfrozen; everything else is
            // left exactly as it stands.
            if let Some(existing) = self.attribute_values.get(name)
                && existing.modification_context == ModificationContext::ApiOnly
                && existing.value != InterpretedValue::Unset
            {
                let unlocked = AttributeValue {
                    modification_context: ModificationContext::Anywhere,
                    silent_when_locked: false,
                    ..existing.clone()
                };

                Arc::make_mut(&mut self.attribute_values).insert(name.to_string(), unlocked);
            }
        }
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
        self.counter_impl(name, seed, false)
    }

    /// Like [`counter`](Self::counter), but the advanced value stays readable
    /// as the attribute of the same name even when that attribute is
    /// *locked* (API-set or a locked built-in). This is the captioning
    /// counter, used for the `<context>-number` of a numbered block.
    ///
    /// Mirrors Asciidoctor's `increment_and_store_counter`: it too advances a
    /// locked counter, and its block attribute entry is replayed onto the
    /// document attributes during conversion, so a locked `example-number`
    /// reads back as its latest counter value (unlike a plain inline
    /// `{counter:…}`, which leaves the locked value in place).
    pub(crate) fn counter_for_caption(&self, name: &str, seed: Option<&str>) -> String {
        self.counter_impl(name, seed, true)
    }

    /// Advances the `name` counter and returns its new value. `seed` supplies
    /// the starting value when the counter has no current value to advance
    /// from.
    ///
    /// A counter reads the current value to produce (and display) the next one.
    /// For an unlocked attribute the advanced value is stored in the readable
    /// [`counter_values`](Self::counter_values) overlay, so a later reference
    /// reads it. For a *locked* attribute – one set via the API, or a locked
    /// built-in such as `max-include-depth` – the write path depends on the
    /// caller:
    ///
    /// * `commit_when_locked` (the captioning counter): the value is stored in
    ///   the readable overlay anyway, matching Asciidoctor's
    ///   `increment_and_store_counter`, whose block attribute entry is replayed
    ///   onto the document attributes during conversion.
    /// * otherwise (an inline `{counter:…}` / `{counter2:…}` directive): the
    ///   running value is kept in the private
    ///   [`locked_counter_values`](Self::locked_counter_values) map instead, so
    ///   the sequence still advances across repeated references while a plain
    ///   reference to the attribute continues to read the locked value. This
    ///   mirrors Asciidoctor's `Document#counter`, which advances `@counters`
    ///   but leaves `@attributes` untouched while the attribute is
    ///   `attribute_locked?`.
    fn counter_impl(&self, name: &str, seed: Option<&str>, commit_when_locked: bool) -> String {
        let use_private_state = !commit_when_locked && self.attribute_is_locked(name);

        // The value to advance from: the private running state first (only
        // populated when it is in use), otherwise the current readable value of
        // the attribute (which, for an unlocked counter, already reflects the
        // overlay).
        let current = if use_private_state {
            self.locked_counter_values.borrow().get(name).cloned()
        } else {
            None
        }
        .or_else(|| match self.attribute_value(name) {
            InterpretedValue::Value(current) if !current.is_empty() => Some(current),
            _ => None,
        });

        let next = match current {
            Some(current) => next_counter_value(&current),
            None => match seed {
                Some(seed) if !seed.is_empty() => seed.to_string(),
                _ => "1".to_string(),
            },
        };

        if use_private_state {
            self.locked_counter_values
                .borrow_mut()
                .insert(name.to_string(), next.clone());
        } else {
            self.counter_values
                .borrow_mut()
                .insert(name.to_string(), next.clone());
        }

        next
    }

    /// Reports whether `name` currently resolves to an attribute that is
    /// *locked* against modification by a counter: it has an effective value
    /// whose [`ModificationContext`] is
    /// [`ApiOnly`](ModificationContext::ApiOnly) – an API-set override or a
    /// locked built-in such as `max-include-depth`.
    ///
    /// This mirrors Asciidoctor's `Document#attribute_locked?`, which is `true`
    /// exactly for an attribute supplied through the API (its
    /// `@attribute_overrides`). It is deliberately *narrower* than the
    /// write-permission check in
    /// [`set_attribute_from_body`](Self::set_attribute_from_body): a
    /// header-only attribute such as an unset `outfilesuffix`
    /// ([`ApiOrHeader`](ModificationContext::ApiOrHeader)) cannot be assigned
    /// from the body, yet a counter *may* advance it (matching Asciidoctor,
    /// where `{counter:outfilesuffix}` moves it while it is not API-locked).
    fn attribute_is_locked(&self, name: &str) -> bool {
        self.effective_attribute(name)
            .is_some_and(|a| a.modification_context == ModificationContext::ApiOnly)
    }
}

/// Whether a `leveloffset` of `offset` leaves at least one syntactic heading
/// level able to land inside the supported section-level range.
///
/// Syntactic heading levels run 0 (`=`) through 5 (`======`) and valid section
/// levels run 1 through 5, so an offset keeps some heading in range only while
/// it stays within `1 - 5 ..= 5 - 0`, i.e. `-4..=5`. Outside that window every
/// heading is clamped, so the offset can never place a heading at its intended
/// level.
fn leveloffset_admits_any_heading(offset: i32) -> bool {
    (-4..=5).contains(&offset)
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

/// Matches every character that Asciidoctor's `sanitize_attribute_name` strips
/// from an attribute name: anything that is not a [word character] (`\w`, i.e.
/// `\p{Word}`) or a hyphen. Mirrors Asciidoctor's `InvalidAttributeNameCharsRx`
/// (`/[^#{CC_WORD}-]/`).
///
/// [word character]: crate::internal::is_word_char
static INVALID_ATTR_NAME_CHARS: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"[^\w-]").unwrap()
});

fn remap_attr_name<N: AsRef<str>>(raw_attr_name: N) -> String {
    // Sanitize the name the way Asciidoctor's `sanitize_attribute_name` does:
    // drop every character that is not a word character or a hyphen, then
    // lower-case the result. This is what lets an attribute entry written as
    // `:Author Initials:` set the `authorinitials` attribute, `:Foo 3^ # -
    // Bar[:` set `foo3-bar`, and `:My frog:` set `myfrog`. Unicode word
    // characters are preserved, so `:café:` sets `café` and `:سمن:` sets `سمن`.
    //
    // The full Unicode case fold (Asciidoctor's `downcase`, not merely ASCII)
    // is what makes an attribute reference case-insensitive: an entry written
    // `:He-Man:` is reachable as `{he-man}` or `{HE-MAN}`. A reference is folded
    // through this same `to_lowercase()` before lookup (see `AttributeReplacer`
    // in `content::substitution_step`), so definition and reference stay
    // symmetric even when a fold expands a character (e.g. `İ` -> `i` + combining
    // dot): both sides land on the identical key.
    let attr_name: String = INVALID_ATTR_NAME_CHARS
        .replace_all(raw_attr_name.as_ref(), "")
        .to_lowercase();

    // Some attribute names have aliases. Remap to the primary name.
    alias_attr_name(attr_name)
}

/// Document attributes that are *flexible*: an API-supplied *set* value is
/// unlocked once the header is parsed so the document body may still toggle it,
/// while an API-supplied [unset] stays locked (see
/// [`unlock_flexible_attributes`](Parser::unlock_flexible_attributes)). Mirrors
/// Asciidoctor's `FLEXIBLE_ATTRIBUTES` constant, currently just `sectnums` (the
/// primary name of the `numbered` alias).
///
/// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
const FLEXIBLE_ATTRIBUTES: [&str; 1] = ["sectnums"];

/// Remaps an attribute name that is a legacy alias to its primary name,
/// returning any other name unchanged.
///
/// `numbered` is a legacy alias for `sectnums`, and `hardbreaks` for
/// `hardbreaks-option`, so setting `numbered` sets `sectnums` (and `numbered!`
/// unsets it), and setting `hardbreaks` sets `hardbreaks-option`. Mirrors both
/// Asciidoctor's `Parser.store_attribute` (which renames a header/body
/// attribute entry before storing it) and its `Document#initialize` (which
/// renames the same two API-supplied attribute overrides: `attr_overrides`
/// `sectnums`/`hardbreaks-option` reassignment). Applying it on both paths is
/// what lets `-a hardbreaks` supplied through the API enable hard line breaks,
/// exactly as `:hardbreaks:` in the header does.
fn alias_attr_name(attr_name: String) -> String {
    match attr_name.as_str() {
        "hardbreaks" => "hardbreaks-option".to_string(),
        "numbered" => "sectnums".to_string(),
        _ => attr_name,
    }
}

/// Returns `true` if `name` is a derived backend-family attribute whose
/// assignment must be rejected *even while it is inactive*, because the flag it
/// would name can become active later in the same parse and the stored override
/// would then shadow the read-only intrinsic:
///
/// * The bare derived values `basebackend` / `filetype` – always resolved on
///   the fly from `backend` (see [`derived_backend_value`]), never stored.
/// * The doctype-keyed flags `backend-<b>-doctype-<d>` /
///   `basebackend-<bb>-doctype-<d>` – the `doctype` component shifts mid-parse
///   (e.g. an AsciiDoc table cell that resets, then changes, its doctype), so
///   an assignment to an inactive one (`backend-html5-doctype-article` while
///   the doctype is `book`) must not be stored where it could shadow the
///   intrinsic once the doctype switches.
///
/// The remaining flag names (`backend-<b>`, `basebackend-<bb>`, `filetype-<f>`,
/// and bare `doctype-<d>`) are deliberately **not** reserved: rejecting them
/// would swallow author-defined attributes such as `:backend-custom:` or
/// `:doctype-draft:` (used as `ifdef` flags), which Asciidoctor keeps. The
/// *active* one of these is still write-protected by the normal permission
/// check, since [`synthesized_attr`] resolves it to a locked intrinsic.
fn is_reserved_derived_attr(name: &str) -> bool {
    is_derived_backend_value(name)
        || ((name.starts_with("backend-") || name.starts_with("basebackend-"))
            && name.contains("-doctype-"))
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
    fn new_matches_default() {
        // `Parser::new()` is a discoverability alias for `Parser::default()`.
        let p = Parser::new();
        assert_eq!(p.attribute_value("foo"), InterpretedValue::Unset);
        assert_eq!(p.safe_mode(), Parser::default().safe_mode());
    }

    mod attribute_state_between_parses {
        use crate::tests::prelude::*;

        #[test]
        fn discovered_header_attribute_does_not_leak() {
            let mut parser = Parser::default();

            // The first document defines `foo`; it is inspectable on the parser
            // once that parse returns.
            let _ = parser.parse(":foo: bar\n\nText.\n");
            assert_eq!(
                parser.attribute_value("foo"),
                InterpretedValue::Value("bar")
            );

            // A second document that never defines `foo` must not observe the
            // first document's assignment.
            let _ = parser.parse("Text.\n");
            assert_eq!(parser.attribute_value("foo"), InterpretedValue::Unset);
        }

        #[test]
        fn discovered_body_attribute_does_not_leak() {
            let mut parser = Parser::default();

            // A body (not header) assignment leaks the same way a header one
            // would if the baseline were not restored.
            let _ = parser.parse("First.\n\n:mode: fast\n\nSecond.\n");
            assert_eq!(
                parser.attribute_value("mode"),
                InterpretedValue::Value("fast")
            );

            let _ = parser.parse("Text.\n");
            assert_eq!(parser.attribute_value("mode"), InterpretedValue::Unset);
        }

        #[test]
        fn configured_baseline_is_restored_each_parse() {
            let mut parser = Parser::default().with_intrinsic_attribute(
                "site",
                "prod",
                ModificationContext::Anywhere,
            );

            // The first document overrides the API-configured value in its body.
            let _ = parser.parse(":site: dev\n\nText.\n");
            assert_eq!(
                parser.attribute_value("site"),
                InterpretedValue::Value("dev")
            );

            // The next parse begins from the configured baseline, not the
            // previous document's override.
            let _ = parser.parse("Text.\n");
            assert_eq!(
                parser.attribute_value("site"),
                InterpretedValue::Value("prod")
            );
        }

        #[test]
        fn reconfiguring_between_parses_updates_baseline() {
            let mut parser = Parser::default();

            let _ = parser.parse("Text.\n");
            assert_eq!(parser.attribute_value("env"), InterpretedValue::Unset);

            // A builder call between parses re-establishes the baseline for every
            // subsequent parse.
            parser = parser.with_intrinsic_attribute("env", "ci", ModificationContext::Anywhere);

            let _ = parser.parse("Text.\n");
            assert_eq!(parser.attribute_value("env"), InterpretedValue::Value("ci"));
        }

        #[test]
        fn leaked_attribute_does_not_affect_rendered_output() {
            let mut parser = Parser::default();

            // The first document defines `who`, so `{who}` resolves for it.
            let doc1 = parser.parse(":who: world\n\nHello {who}.\n");
            assert_eq!(rendered_paragraphs(&doc1), vec!["Hello world."]);

            // The second document does not define `who`; without the baseline
            // restore, `{who}` would still resolve to "world". Instead it stays
            // an unresolved literal reference.
            let doc2 = parser.parse("Hello {who}.\n");
            assert_eq!(rendered_paragraphs(&doc2), vec!["Hello {who}."]);
        }
    }

    mod leading_byte_order_mark {
        use crate::tests::prelude::*;

        #[test]
        fn bom_before_header_yields_title() {
            // A UTF-8 BOM precedes the document header. It must be stripped so
            // the `= ` title line is recognized rather than misparsed as a
            // paragraph.
            let doc = Parser::default().parse("\u{feff}= My Title\n\nbody");

            assert_eq!(doc.header().title(), Some("My Title"));
            assert_eq!(rendered_paragraphs(&doc), vec!["body"]);
        }

        #[test]
        fn bom_before_paragraph_is_stripped() {
            // With no header, the BOM must still be removed so it does not
            // become the first character of the paragraph's content.
            let doc = Parser::default().parse("\u{feff}Hello.\n");

            assert_eq!(rendered_paragraphs(&doc), vec!["Hello."]);
        }

        #[test]
        fn only_a_single_leading_bom_is_stripped() {
            // Only one leading BOM is stripped; a second U+FEFF is ordinary
            // content and survives into the paragraph text (matching
            // Asciidoctor).
            let doc = Parser::default().parse("\u{feff}\u{feff}Hello.\n");

            assert_eq!(rendered_paragraphs(&doc), vec!["\u{feff}Hello."]);
        }

        #[test]
        fn bom_precedes_front_matter_handling() {
            // The BOM is stripped before front-matter detection, so a `---`
            // fence that immediately follows the mark still opens front matter
            // when `skip-front-matter` is set.
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(
                    "skip-front-matter",
                    true,
                    ModificationContext::ApiOnly,
                )
                .parse("\u{feff}---\ntitle: Doc\n---\n\n= My Title\n\nbody");

            assert_eq!(
                doc.attribute_value("front-matter"),
                InterpretedValue::Value("title: Doc")
            );
            assert_eq!(doc.header().title(), Some("My Title"));
        }
    }

    mod remap_attr_name {
        use super::super::remap_attr_name;

        #[test]
        fn strips_non_word_and_lower_cases_ascii() {
            assert_eq!(remap_attr_name("Foo Bar"), "foobar");
            assert_eq!(remap_attr_name("Foo 3^ # - Bar["), "foo3-bar");
            assert_eq!(remap_attr_name("My frog"), "myfrog");
        }

        #[test]
        fn remaps_legacy_aliases() {
            // `hardbreaks` and `numbered` are legacy aliases remapped to their
            // primary names, matching Asciidoctor's `Parser.store_attribute`.
            assert_eq!(remap_attr_name("hardbreaks"), "hardbreaks-option");
            assert_eq!(remap_attr_name("Hardbreaks"), "hardbreaks-option");
            assert_eq!(remap_attr_name("numbered"), "sectnums");
        }

        #[test]
        fn preserves_unicode_word_characters() {
            // Unicode letters and digits are word characters, so they survive
            // sanitization; the `{café}` / `{سمن}` references then resolve.
            assert_eq!(remap_attr_name("café"), "café");
            assert_eq!(remap_attr_name("سمن"), "سمن");
        }

        #[test]
        fn preserves_marks_and_join_controls() {
            // `\p{Word}` includes combining marks and join controls, so a
            // decomposed name and a name embedding a ZWNJ are not mangled.
            let decomposed = "cafe\u{301}";
            assert_eq!(remap_attr_name(decomposed), decomposed);

            let with_zwnj = "\u{645}\u{200c}\u{646}";
            assert_eq!(remap_attr_name(with_zwnj), with_zwnj);
        }

        #[test]
        fn folds_case_with_full_unicode() {
            // The name is folded with the full Unicode `to_lowercase()`, so a
            // reference lookup is case-insensitive. A fold that expands a
            // character (`İ` -> `i` + U+0307 combining dot above) is harmless:
            // an attribute reference is folded through the same function, so
            // definition and reference still land on the identical key.
            assert_eq!(remap_attr_name("He-Man"), "he-man");
            assert_eq!(remap_attr_name("İstanbul"), "i\u{307}stanbul");
            assert_eq!(remap_attr_name("İstanbul"), "İstanbul".to_lowercase());
        }
    }

    #[test]
    fn attribute_reference_resolves_case_insensitively() {
        // A reference is folded with the same Unicode `to_lowercase()` used to
        // store the name, so any casing of the reference resolves the entry.
        let doc = Parser::default().parse(":He-Man: the foe\n\n{he-man} / {HE-MAN} / {He-Man}");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec!["the foe / the foe / the foe"]
        );
    }

    #[test]
    fn attribute_reference_case_fold_round_trips_when_it_expands() {
        // `İ` folds to `i` + U+0307 under `to_lowercase()`. Because both the
        // definition and the reference fold through that same function, the
        // entry stays reachable by its own spelling despite the expansion.
        let doc = Parser::default().parse(":İ: dotted\n\n{İ}");
        assert_eq!(rendered_paragraphs(&doc), vec!["dotted"]);
    }

    #[test]
    fn unicode_attribute_reference_resolves_in_preprocessor() {
        // The preprocessor (conditional directives, include targets) resolves
        // `{name}` references with the same Unicode word-character class as the
        // main substitution pass, so a Unicode-named attribute drives an
        // `ifeval` condition. See #726.
        let doc = Parser::default()
            .parse(":café: yes\n\nifeval::[\"{café}\" == \"yes\"]\nShown.\nendif::[]");
        assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);
    }

    #[test]
    fn case_insensitive_attribute_reference_resolves_in_preprocessor() {
        // The preprocessor folds a `{name}` reference the same way the main
        // substitution pass does, so a mismatched-case reference still drives an
        // `ifeval` condition.
        let doc = Parser::default()
            .parse(":Answer: yes\n\nifeval::[\"{answer}\" == \"yes\"]\nShown.\nendif::[]");
        assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);
    }

    #[test]
    fn owned_cell_warning_is_recorded_only_inside_an_owned_cell_source() {
        use std::rc::Rc;

        use crate::{
            parser::{SourceLine, SourceMap},
            warnings::WarningType,
        };

        let mut p = Parser::default();

        // Outside an owned cell source there is no map to resolve against, so a
        // recorded warning has no origin and is dropped rather than queued.
        assert!(!p.is_in_owned_cell_source());
        p.record_owned_cell_warning(
            1,
            WarningType::IncludeFileNotFound("x.adoc".to_owned()),
            None,
        );
        assert!(p.take_owned_cell_warnings().is_empty());

        // An explicit origin override is queued even without a cell source map.
        p.record_owned_cell_warning(
            1,
            WarningType::UnterminatedConditionalDirective("ifdef::foo[]".to_owned()),
            Some(SourceLine(Some("inc.adoc".to_owned()), 3)),
        );
        let overridden = p.take_owned_cell_warnings();
        let [overridden] = overridden.as_slice() else {
            panic!("expected exactly one recorded warning, got {overridden:?}");
        };
        assert_eq!(
            overridden.origin,
            SourceLine(Some("inc.adoc".to_owned()), 3)
        );

        // Publish a cell source map (output line 1 came from `cell.adoc` line 2,
        // the way the preprocessor would record an include-expanded cell).
        let mut sm = SourceMap::default();
        sm.append(1, Some("cell.adoc"), 2, crate::parser::Fidelity::Verbatim);
        p.push_owned_cell_source_map(Rc::new(sm));
        assert!(p.is_in_owned_cell_source());

        // Now the same call resolves the line to its origin and queues the
        // warning with that pre-resolved (file, line).
        p.record_owned_cell_warning(
            1,
            WarningType::IncludeFileNotFound("y.adoc".to_owned()),
            None,
        );
        let recorded = p.take_owned_cell_warnings();
        let [recorded] = recorded.as_slice() else {
            panic!("expected exactly one recorded warning, got {recorded:?}");
        };
        assert_eq!(recorded.origin, SourceLine(Some("cell.adoc".to_owned()), 2));
        assert_eq!(
            recorded.warning,
            WarningType::IncludeFileNotFound("y.adoc".to_owned())
        );

        // Taking drains the buffer, and popping restores the not-in-owned-cell
        // state.
        assert!(p.take_owned_cell_warnings().is_empty());
        p.pop_owned_cell_source_map();
        assert!(!p.is_in_owned_cell_source());
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
    fn with_intrinsic_attribute_remaps_legacy_aliases() {
        // An API-supplied `hardbreaks` (Asciidoctor's `-a hardbreaks`) is a
        // legacy alias remapped to `hardbreaks-option`, exactly as the same
        // attribute written as a header entry (`:hardbreaks:`) is. Without the
        // remap the API form is stored verbatim as `hardbreaks` and the
        // paragraph's post-replacement check (which consults
        // `hardbreaks-option`) never sees it.
        let mut p = Parser::default().with_intrinsic_attribute(
            "hardbreaks",
            "",
            ModificationContext::Anywhere,
        );

        assert!(p.is_attribute_set("hardbreaks-option"));

        // The end-to-end effect: each unwrapped line gains a hard line break.
        let doc = p.parse("First line\nSecond line");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec!["First line<br>\nSecond line"]
        );
    }

    #[test]
    fn api_set_flexible_attribute_is_unlocked_after_the_header() {
        // An API-*set* `sectnums` (here via the `numbered` alias) is a flexible
        // attribute: it seeds numbering on, but is unlocked once the header is
        // parsed so a body `:sectnums!:` still takes effect. Modeled as the
        // html5 converter applies it: an `ApiOnly` override on `numbered`.
        let mut p = Parser::default().with_intrinsic_attribute(
            "numbered",
            "",
            ModificationContext::ApiOnly,
        );

        // Before any parse the alias is stored, locked, under `sectnums`.
        assert!(p.is_attribute_set("sectnums"));

        // A body `:sectnums!:` turns numbering back off for the sections that
        // follow it: the unlock let the assignment through.
        let doc = p.parse("= Title\n\n== On\n\n:sectnums!:\n\n== Off");
        let nums: Vec<Option<String>> = crate::tests::prelude::all_sections(&doc)
            .iter()
            .map(|s| s.section_number().map(|n| n.to_string()))
            .collect();

        assert_eq!(nums, vec![Some("1".to_string()), None]);
    }

    #[test]
    fn api_unset_flexible_attribute_stays_locked() {
        // An API-*unset* `sectnums` (here via `numbered!`, i.e. the alias set to
        // `false`) stays locked: a body `:sectnums:` cannot re-enable numbering.
        let mut p = Parser::default().with_intrinsic_attribute_bool(
            "numbered",
            false,
            ModificationContext::ApiOnly,
        );

        assert!(!p.is_attribute_set("sectnums"));

        let doc = p.parse("= Title\n\n:sectnums:\n\n== Still Off");
        let nums: Vec<Option<String>> = crate::tests::prelude::all_sections(&doc)
            .iter()
            .map(|s| s.section_number().map(|n| n.to_string()))
            .collect();

        assert_eq!(nums, vec![None]);
    }

    // Under `SafeMode::Server` or greater, `docdir` reads as empty and
    // `docfile` is relativized against `docdir`; see #735 and the ported
    // upstream tests in `tests/asciidoctor_rb/attributes_test.rs`. These cover
    // crate-specific edge cases not exercised by the single upstream test.
    #[test]
    fn masks_docdir_and_docfile_under_secure_mode() {
        // Secure (the default) is stricter than Server, so masking also applies.
        let p = Parser::default()
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sample.adoc",
                ModificationContext::ApiOnly,
            );
        assert_eq!(p.safe_mode(), SafeMode::Secure);
        assert_eq!(p.attribute_value("docdir"), InterpretedValue::Value(""));
        assert_eq!(
            p.attribute_value("docfile"),
            InterpretedValue::Value("sample.adoc")
        );

        // The masked `docdir` is still a *set* (present) attribute.
        assert!(p.is_attribute_set("docdir"));
        assert!(p.has_attribute("docfile"));
    }

    #[test]
    fn relativizes_docfile_in_a_subdirectory_of_docdir() {
        // A `docfile` nested below `docdir` keeps its sub-path relative to
        // `docdir` (not merely its base name), matching Asciidoctor's
        // `docfile[(docdir.length + 1)..-1]` slice.
        let p = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sub/sample.adoc",
                ModificationContext::ApiOnly,
            );
        assert_eq!(
            p.attribute_value("docfile"),
            InterpretedValue::Value("sub/sample.adoc")
        );
    }

    #[test]
    fn relativizes_docfile_not_under_docdir_to_its_basename() {
        // An inconsistent `docdir` / `docfile` pairing (docfile not under
        // docdir) must not be truncated at an unrelated byte offset; it
        // relativizes to the base name instead (see #735 review feedback).
        let p = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/different/file.adoc",
                ModificationContext::ApiOnly,
            );
        assert_eq!(
            p.attribute_value("docfile"),
            InterpretedValue::Value("file.adoc")
        );
    }

    #[test]
    fn docfile_without_docdir_falls_back_to_basename_under_server_mode() {
        let p = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sample.adoc",
                ModificationContext::ApiOnly,
            );
        assert_eq!(
            p.attribute_value("docfile"),
            InterpretedValue::Value("sample.adoc")
        );
    }

    #[test]
    fn does_not_mask_docdir_and_docfile_below_server_mode() {
        // Below Server, the API-provided values pass through verbatim.
        let p = Parser::default()
            .with_safe_mode(SafeMode::Safe)
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sample.adoc",
                ModificationContext::ApiOnly,
            );
        assert_eq!(
            p.attribute_value("docdir"),
            InterpretedValue::Value("/some/dir")
        );
        assert_eq!(
            p.attribute_value("docfile"),
            InterpretedValue::Value("/some/dir/sample.adoc")
        );
    }

    #[test]
    fn unset_docdir_and_docfile_stay_missing_under_server_mode() {
        // Masking never conjures a value for an attribute that was never set, so
        // a reference to an unset `docdir` / `docfile` still resolves as missing.
        let p = Parser::default().with_safe_mode(SafeMode::Server);
        assert_eq!(p.attribute_value("docdir"), InterpretedValue::Unset);
        assert_eq!(p.attribute_value("docfile"), InterpretedValue::Unset);
        assert!(!p.has_attribute("docdir"));
        assert!(!p.has_attribute("docfile"));
    }

    #[test]
    fn leaves_non_string_docdir_and_docfile_untouched_under_server_mode() {
        // A `docdir` / `docfile` present as a boolean flag (not a path string)
        // carries no host path to leak, so the masking has nothing to blank or
        // relativize and leaves the (empty) `Set` value as-is.
        let p = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("docdir", true, ModificationContext::ApiOnly)
            .with_intrinsic_attribute_bool("docfile", true, ModificationContext::ApiOnly);
        assert_eq!(p.attribute_value("docdir"), InterpretedValue::Set);
        assert_eq!(p.attribute_value("docfile"), InterpretedValue::Set);
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
    fn asciidoc_parser_version_is_predefined() {
        // The crate predefines `asciidoc-parser-version` with its own version
        // (the parser-specific counterpart of Ruby Asciidoctor's
        // `asciidoctor-version` intrinsic).
        let mut parser = Parser::default();

        assert_eq!(
            parser.attribute_value("asciidoc-parser-version"),
            InterpretedValue::Value(env!("CARGO_PKG_VERSION"))
        );

        // The value is available to attribute references and `ifeval`
        // expressions in document content.
        let doc = parser.parse(concat!(
            "= Title\n",
            "\n",
            "ifeval::['{asciidoc-parser-version}' >= '0.1.0']\n",
            "v{asciidoc-parser-version}\n",
            "endif::[]\n",
        ));

        assert_eq!(
            rendered_paragraphs(&doc),
            vec![format!("v{}", env!("CARGO_PKG_VERSION"))]
        );
    }

    #[test]
    fn asciidoc_parser_version_is_locked() {
        // The parser version describes the processor itself, so a document
        // assignment is rejected with a warning and the built-in value stays
        // in place.
        let mut parser = Parser::default();

        let doc = parser.parse(":asciidoc-parser-version: 99.99.99");

        assert_eq!(
            doc.warnings().next().unwrap().warning,
            WarningType::AttributeValueIsLocked("asciidoc-parser-version".to_owned())
        );

        assert_eq!(
            parser.attribute_value("asciidoc-parser-version"),
            InterpretedValue::Value(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn asciidoctor_version_is_predefined() {
        // The crate predefines `asciidoctor-version` with the Asciidoctor
        // release whose behavior it implements, so documents written against
        // Asciidoctor's own intrinsic behave the same here.
        let mut parser = Parser::default();

        assert_eq!(
            parser.attribute_value("asciidoctor-version"),
            InterpretedValue::Value(crate::ASCIIDOCTOR_VERSION)
        );

        // The value is available to `ifdef` gating and to attribute references
        // and `ifeval` expressions in document content.
        let doc = parser.parse(concat!(
            "= Title\n",
            "\n",
            "ifdef::asciidoctor-version[]\n",
            "ifeval::['{asciidoctor-version}' >= '0.1.0']\n",
            "v{asciidoctor-version}\n",
            "endif::[]\n",
            "endif::[]\n",
        ));

        assert_eq!(
            rendered_paragraphs(&doc),
            vec![format!("v{}", crate::ASCIIDOCTOR_VERSION)]
        );
    }

    #[test]
    fn asciidoctor_version_is_locked() {
        // Like its `asciidoc-parser-version` companion, this describes the
        // processor itself, so a document assignment is rejected with a warning
        // and the built-in value stays in place.
        let mut parser = Parser::default();

        let doc = parser.parse(":asciidoctor-version: 99.99.99");

        assert_eq!(
            doc.warnings().next().unwrap().warning,
            WarningType::AttributeValueIsLocked("asciidoctor-version".to_owned())
        );

        assert_eq!(
            parser.attribute_value("asciidoctor-version"),
            InterpretedValue::Value(crate::ASCIIDOCTOR_VERSION)
        );
    }

    #[test]
    fn asciidoctor_flag_is_predefined() {
        // The crate predefines the always-set `asciidoctor` boolean flag, so a
        // document guarding Asciidoctor-only content with `ifdef::asciidoctor[]`
        // behaves the same here. A `////` comment block containing a directive
        // that would corrupt it once the flag is defined must stay untouched
        // (see issue #810).
        let mut parser = Parser::default();

        assert_eq!(parser.attribute_value("asciidoctor"), InterpretedValue::Set);

        let doc = parser.parse(concat!(
            "= Title\n",
            "\n",
            "ifdef::asciidoctor[]\n",
            "shown when asciidoctor is set\n",
            "endif::[]\n",
            "\n",
            "////\n",
            "ifdef::asciidoctor[////]\n",
            "////\n",
            "\n",
            "line after comment block\n",
        ));

        assert_eq!(
            rendered_paragraphs(&doc),
            vec![
                "shown when asciidoctor is set".to_owned(),
                "line after comment block".to_owned(),
            ]
        );
    }

    #[test]
    fn asciidoctor_flag_is_locked() {
        // Like the version intrinsics, the flag describes the processor itself,
        // so a document assignment is rejected with a warning and the built-in
        // value stays in place.
        let mut parser = Parser::default();

        let doc = parser.parse(":asciidoctor: 99.99.99");

        assert_eq!(
            doc.warnings().next().unwrap().warning,
            WarningType::AttributeValueIsLocked("asciidoctor".to_owned())
        );

        assert_eq!(parser.attribute_value("asciidoctor"), InterpretedValue::Set);
    }

    #[test]
    fn asciidoc_parser_version_distinguishes_the_two_processors() {
        // Both version intrinsics are defined, so a document tells the
        // processors apart via `asciidoc-parser-version`, which Ruby
        // Asciidoctor does not define.
        let mut parser = Parser::default();

        let doc = parser.parse(concat!(
            "= Title\n",
            "\n",
            "ifdef::asciidoc-parser-version[]\n",
            "This is asciidoc-parser.\n",
            "endif::[]\n",
        ));

        assert_eq!(rendered_paragraphs(&doc), vec!["This is asciidoc-parser."]);
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

        fn render_quoted_substitution(
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
        let block = doc.child_blocks().next().unwrap();

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

        let block = doc.child_blocks().next().unwrap();
        let Block::Simple(simple_block) = block else {
            panic!("Expected simple block, got: {block:?}");
        };

        assert_eq!(simple_block.content().rendered(), "test.[FOOTNOTE:missing]");
    }

    /// A custom [`PathResolver`](crate::parser::PathResolver) that rewrites
    /// every asset target under a fixed content root, ignoring the start path.
    /// Stands in for a host (Antora/Zola-style) that maps targets through a
    /// virtual filesystem or URL scheme.
    #[derive(Debug)]
    struct CdnPathResolver;

    impl crate::parser::PathResolver for CdnPathResolver {
        fn web_path(&self, target: &str, _start: Option<&str>) -> String {
            format!("https://cdn.example.com/{target}")
        }
    }

    #[test]
    fn with_path_resolver() {
        let mut parser = Parser::default().with_path_resolver(CdnPathResolver);

        // An inline image's `src` is resolved through the path resolver, so the
        // custom resolver should rewrite it under the content root.
        let doc = parser.parse("image:tiger.png[tiger]");

        let block = doc.child_blocks().next().unwrap();
        let Block::Simple(simple_block) = block else {
            panic!("Expected simple block, got: {block:?}");
        };

        assert_eq!(
            simple_block.content().rendered(),
            r#"<span class="image"><img src="https://cdn.example.com/tiger.png" alt="tiger"></span>"#
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

    mod notitle_showtitle_linkage {
        use crate::{
            blocks::{Block, FindBlocks},
            document::InterpretedValue,
            parser::{ModificationContext, Parser},
        };

        // Asciidoctor asciidoctor/asciidoctor#3804: `notitle` and `showtitle`
        // are two spellings of one title-visibility toggle, wired as inverses.
        // Assigning either updates the partner so the resolved document carries
        // one consistent signal – following Asciidoctor's hash semantics, where
        // turning the toggle *on* sets one spelling and *removes* the other.

        fn parse_header(entries: &str) -> Parser {
            let mut parser = Parser::default();
            let _ = parser.parse(&format!("= Title\n{entries}\n\nbody"));
            parser
        }

        #[test]
        fn header_showtitle_set_unsets_notitle() {
            // `:showtitle:` => notitle removed (absent).
            let parser = parse_header(":showtitle:");
            assert_eq!(parser.attribute_value("showtitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("notitle"));
        }

        #[test]
        fn header_showtitle_unset_sets_notitle() {
            // `:!showtitle:` => notitle set.
            let parser = parse_header(":!showtitle:");
            assert!(!parser.is_attribute_set("showtitle"));
            assert_eq!(parser.attribute_value("notitle"), InterpretedValue::Set);
            assert!(parser.is_attribute_set("notitle"));
        }

        #[test]
        fn header_notitle_set_unsets_showtitle() {
            // `:notitle:` => showtitle removed (absent).
            let parser = parse_header(":notitle:");
            assert_eq!(parser.attribute_value("notitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("showtitle"));
        }

        #[test]
        fn header_notitle_unset_sets_showtitle() {
            // `:!notitle:` => showtitle set. This is the case called out in the
            // issue: a consumer keying off `showtitle` now sees a signal.
            let parser = parse_header(":!notitle:");
            assert!(!parser.is_attribute_set("notitle"));
            assert_eq!(parser.attribute_value("showtitle"), InterpretedValue::Set);
            assert!(parser.is_attribute_set("showtitle"));
        }

        #[test]
        fn last_assignment_wins() {
            // Each assignment rewrites the partner, so whichever is assigned
            // last decides the resolved toggle.
            let parser = parse_header(":notitle:\n:showtitle:");
            assert_eq!(parser.attribute_value("showtitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("notitle"));

            let parser = parse_header(":showtitle:\n:notitle:");
            assert_eq!(parser.attribute_value("notitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("showtitle"));
        }

        #[test]
        fn body_assignment_is_linked() {
            // A body attribute entry links the partner just as a header entry
            // does.
            let mut parser = Parser::default();
            let _ = parser.parse("= Title\n\nintro\n\n:notitle:\n\nmore");
            assert_eq!(parser.attribute_value("notitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("showtitle"));
        }

        #[test]
        fn api_assignment_is_linked() {
            // Setting either attribute via the API links the partner, matching
            // Asciidoctor's `attributes: { 'notitle!' => '' }` etc.
            let parser = Parser::default().with_intrinsic_attribute_bool(
                "notitle",
                true,
                ModificationContext::Anywhere,
            );
            assert_eq!(parser.attribute_value("notitle"), InterpretedValue::Set);
            assert!(!parser.has_attribute("showtitle"));

            let parser = Parser::default().with_intrinsic_attribute_bool(
                "notitle",
                false,
                ModificationContext::Anywhere,
            );
            assert!(!parser.is_attribute_set("notitle"));
            assert_eq!(parser.attribute_value("showtitle"), InterpretedValue::Set);
        }

        #[test]
        fn turning_the_toggle_on_leaves_no_partner_tombstone() {
            // `:notitle:` removes `showtitle` outright rather than leaving an
            // unset tombstone, so a `{showtitle}` reference stays literal (as it
            // would with the attribute absent) instead of resolving to an empty
            // string. This guards the interaction flagged in review.
            let parser = parse_header(":notitle:");
            assert!(!parser.has_attribute("showtitle"));

            let mut parser = Parser::default();
            let doc = parser.parse("= Title\n:notitle:\n\n{showtitle}");
            let block = doc.child_blocks().next().unwrap();
            let Block::Simple(simple_block) = block else {
                panic!("expected a simple block");
            };
            assert_eq!(simple_block.content().rendered(), "{showtitle}");
        }

        #[test]
        fn unrelated_attributes_are_untouched() {
            // A document that never assigns either spelling leaves both absent –
            // the linkage is a no-op for every other attribute.
            let parser = parse_header(":sectnums:");
            assert!(!parser.has_attribute("notitle"));
            assert!(!parser.has_attribute("showtitle"));
        }
    }

    mod derived_backend_family_attrs {
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

        #[test]
        fn default_backend_family_is_materialized() {
            let parser = Parser::default();

            // The default backend is `html5`; its whole derived family resolves
            // to queryable document attributes (empty-valued flags plus the
            // `backend` / `basebackend` / `filetype` values).
            for (name, value) in [
                ("backend", "html5"),
                ("backend-html5", ""),
                ("basebackend", "html"),
                ("basebackend-html", ""),
                ("filetype", "html"),
                ("filetype-html", ""),
                ("doctype-article", ""),
                ("backend-html5-doctype-article", ""),
                ("basebackend-html-doctype-article", ""),
            ] {
                assert!(parser.has_attribute(name), "missing {name:?}");
                assert!(parser.is_attribute_set(name), "not set: {name:?}");
                assert_eq!(
                    parser.attribute_value(name),
                    InterpretedValue::Value(value.to_string()),
                    "unexpected value for {name:?}"
                );
            }
        }

        #[test]
        fn family_tracks_a_non_html_backend() {
            // Setting a different backend re-derives the whole family from it
            // (basebackend strips the trailing digits, filetype maps through the
            // Asciidoctor extension table), and the inactive `html5` flags fall
            // away.
            let doc = Parser::default().parse(":backend: docbook5\n\nbody");

            assert_eq!(
                doc.attribute_value("backend"),
                InterpretedValue::Value("docbook5".to_string())
            );
            assert_eq!(
                doc.attribute_value("basebackend"),
                InterpretedValue::Value("docbook".to_string())
            );
            assert_eq!(
                doc.attribute_value("filetype"),
                InterpretedValue::Value("xml".to_string())
            );
            assert!(doc.has_attribute("backend-docbook5"));
            assert!(doc.has_attribute("basebackend-docbook"));
            assert!(doc.has_attribute("backend-docbook5-doctype-article"));

            // The derived values report as set through the post-parse
            // `Document` (snapshot) reader, not just the live parser.
            assert!(doc.is_attribute_set("basebackend"));
            assert!(doc.is_attribute_set("filetype"));

            // The html5 flags are no longer active.
            assert!(!doc.has_attribute("backend-html5"));
            assert!(!doc.has_attribute("basebackend-html"));
            assert!(!doc.has_attribute("backend-html5-doctype-article"));
        }

        #[test]
        fn derived_value_and_flag_attributes_are_read_only() {
            // `basebackend` / `filetype` and the derived flag namespace are
            // read-only intrinsics; a document assignment is silently ignored and
            // the synthesized value stands.
            let doc = Parser::default().parse(
                ":basebackend: custom\n:filetype: custom\n:backend-html5: custom\n:doctype-article: custom\n\nbody",
            );

            assert_eq!(
                doc.attribute_value("basebackend"),
                InterpretedValue::Value("html".to_string())
            );
            assert_eq!(
                doc.attribute_value("filetype"),
                InterpretedValue::Value("html".to_string())
            );
            assert_eq!(
                doc.attribute_value("backend-html5"),
                InterpretedValue::Value(String::new())
            );
            assert_eq!(
                doc.attribute_value("doctype-article"),
                InterpretedValue::Value(String::new())
            );
        }

        #[test]
        fn custom_prefixed_flags_stay_assignable() {
            // Author-defined attributes that share a derived-family prefix but
            // name no active flag (and are not the doctype-keyed namespace) are
            // kept, not swallowed by the read-only reservation, so they stay
            // visible to `ifdef` / attribute references – matching Asciidoctor.
            let doc = Parser::default().parse(
                ":backend-custom: enabled\n:basebackend-custom: on\n:filetype-custom: yes\n:doctype-draft: 1\n\nbody",
            );

            for (name, value) in [
                ("backend-custom", "enabled"),
                ("basebackend-custom", "on"),
                ("filetype-custom", "yes"),
                ("doctype-draft", "1"),
            ] {
                assert!(doc.has_attribute(name), "missing {name:?}");
                assert_eq!(
                    doc.attribute_value(name),
                    InterpretedValue::Value(value.to_string()),
                    "unexpected value for {name:?}"
                );
            }
        }

        #[test]
        fn unset_backend_makes_the_family_absent() {
            // Explicitly unsetting `backend` leaves nothing to derive from, so
            // `basebackend` / `filetype` and the backend-keyed flags are absent
            // rather than resolving to empty traits or degenerate `backend-` /
            // `filetype-` names.
            let doc = Parser::default().parse(":backend!:\n\nbody");

            assert_eq!(doc.attribute_value("backend"), InterpretedValue::Unset);
            for name in ["basebackend", "filetype"] {
                assert!(!doc.has_attribute(name), "unexpectedly present: {name:?}");
                assert!(!doc.is_attribute_set(name), "unexpectedly set: {name:?}");
                assert_eq!(doc.attribute_value(name), InterpretedValue::Unset);
            }

            // No degenerate empty-suffix flags, and the html5 flags are gone.
            for name in [
                "backend-",
                "basebackend-",
                "filetype-",
                "backend-html5",
                "basebackend-html",
            ] {
                assert!(!doc.has_attribute(name), "unexpectedly present: {name:?}");
            }

            // The doctype-only flag does not depend on `backend`, so it remains.
            assert!(doc.has_attribute("doctype-article"));
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

    /// Coverage for the time-dependent document attributes (`docdate`,
    /// `doctime`, `docdatetime`, `docyear`, and their `local*` siblings) that
    /// is *not* a direct port of Asciidoctor's Ruby tests: the injectable
    /// clock ([`Parser::with_reference_time`] /
    /// [`Parser::with_input_mtime`]), and resolution *during* a parse (a
    /// `{docdate}` reference or an `ifdef::docdate[]` directive) rather
    /// than off the finished document.
    ///
    /// The direct Ruby ports live alongside the vendored suite in
    /// `tests/asciidoctor_rb/document_test.rs`.
    mod datetime_attributes {
        use crate::{parser::ReferenceTime, tests::prelude::*};

        #[test]
        fn pins_local_attributes_with_reference_time() {
            // The injectable clock (this crate's stable-output mechanism) pins
            // the `local*` attributes, which Asciidoctor derives from
            // `::Time.now`.
            let doc = Parser::default()
                .with_reference_time(ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, 6 * 3600))
                .parse("");

            assert_eq!(
                doc.attribute_value("localdate"),
                InterpretedValue::Value("2019-01-02")
            );
            assert_eq!(
                doc.attribute_value("localyear"),
                InterpretedValue::Value("2019")
            );
            assert_eq!(
                doc.attribute_value("localtime"),
                InterpretedValue::Value("03:04:05 +0600")
            );
            assert_eq!(
                doc.attribute_value("localdatetime"),
                InterpretedValue::Value("2019-01-02 03:04:05 +0600")
            );
        }

        #[test]
        fn resolves_date_attributes_referenced_in_the_document_body() {
            // A `{docdate}` reference resolves the attribute on demand through
            // the parser (during substitution), not off the finished document
            // snapshot.
            let doc = Parser::default()
                .with_reference_time(ReferenceTime::from_unix_timestamp(1_420_106_400))
                .parse("docdate={docdate} docyear={docyear} docdatetime={docdatetime}");

            assert_eq!(
                rendered_paragraphs(&doc),
                vec![
                    "docdate=2015-01-01 docyear=2015 docdatetime=2015-01-01 10:00:00 UTC"
                        .to_string()
                ]
            );
        }

        #[test]
        fn conditional_directive_sees_a_computed_date_attribute() {
            // `ifdef` queries `is_attribute_set`, which must report the computed
            // `docdate` as set.
            let doc = Parser::default()
                .with_reference_time(ReferenceTime::from_unix_timestamp(1_420_106_400))
                .parse("ifdef::docdate[present]");

            assert_eq!(rendered_paragraphs(&doc), vec!["present".to_string()]);
        }

        #[test]
        fn an_explicit_doctime_feeds_the_computed_docdatetime() {
            // An explicit `doctime` (a stored value) supplies the time portion
            // of the computed `docdatetime`, both when referenced in the body
            // and when read off the document.
            let mut parser = Parser::default()
                .with_reference_time(ReferenceTime::from_unix_timestamp(1_420_106_400))
                .with_intrinsic_attribute(
                    "doctime",
                    "09:09:09-0500",
                    ModificationContext::ApiOrHeader,
                );
            let doc = parser.parse("at {docdatetime}");

            assert_eq!(
                rendered_paragraphs(&doc),
                vec!["at 2015-01-01 09:09:09-0500".to_string()]
            );
            assert_eq!(
                doc.attribute_value("docdatetime"),
                InterpretedValue::Value("2015-01-01 09:09:09-0500")
            );
        }

        #[test]
        fn an_unset_doctime_falls_back_to_the_reference_time() {
            // An explicitly unset `doctime` is treated as absent, so
            // `docdatetime` falls back to the reference instant's time.
            let mut parser = Parser::default()
                .with_reference_time(ReferenceTime::from_unix_timestamp(1_420_106_400))
                .with_intrinsic_attribute_bool("doctime", false, ModificationContext::ApiOrHeader);
            let doc = parser.parse("at {docdatetime}");

            assert_eq!(
                rendered_paragraphs(&doc),
                vec!["at 2015-01-01 10:00:00 UTC".to_string()]
            );
            assert_eq!(
                doc.attribute_value("docdatetime"),
                InterpretedValue::Value("2015-01-01 10:00:00 UTC")
            );
        }

        #[test]
        fn a_value_less_doctime_reads_as_an_empty_time() {
            // A value-less `doctime` (set, but with no value) contributes an
            // empty time, leaving a trailing space in the computed
            // `docdatetime`.
            let mut parser = Parser::default()
                .with_reference_time(ReferenceTime::from_unix_timestamp(1_420_106_400))
                .with_intrinsic_attribute_bool("doctime", true, ModificationContext::ApiOrHeader);
            let doc = parser.parse("x{docdatetime}x");

            assert_eq!(rendered_paragraphs(&doc), vec!["x2015-01-01 x".to_string()]);
            assert_eq!(
                doc.attribute_value("docdatetime"),
                InterpretedValue::Value("2015-01-01 ")
            );
        }
    }

    // Crate-native `skip-front-matter` cases (no Asciidoctor analog) covering
    // edge conditions beyond the reader-suite ports in
    // `tests::asciidoctor_rb::reader_test`. See [`Parser::skip_front_matter`].
    mod skip_front_matter {
        use crate::tests::prelude::*;

        #[test]
        fn crlf_line_endings() {
            // The front-matter delimiters are matched after a CRLF line ending
            // is stripped, and the captured `front-matter` value is likewise
            // chomped, so a document with `\r\n` line endings is handled the
            // same as one with bare `\n`.
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(
                    "skip-front-matter",
                    true,
                    ModificationContext::ApiOnly,
                )
                .parse("---\r\nlayout: post\r\ntitle: Document Title\r\n---\r\n= Document Title\r\nAuthor Name\r\n\r\npreamble\r\n");

            assert_eq!(
                doc.attribute_value("front-matter"),
                InterpretedValue::Value("layout: post\ntitle: Document Title")
            );
            assert_eq!(doc.header().title(), Some("Document Title"));
            assert_eq!(doc.header().title_source().unwrap().line(), 5);
        }

        #[test]
        fn first_line_is_not_a_delimiter() {
            // With `skip-front-matter` set but no opening `---` on the first
            // line, there is nothing to skip: the document parses normally and
            // no `front-matter` attribute is recorded.
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(
                    "skip-front-matter",
                    true,
                    ModificationContext::ApiOnly,
                )
                .parse("= Document Title\nAuthor Name\n\npreamble\n");

            assert_eq!(doc.attribute_value("front-matter"), InterpretedValue::Unset);
            assert_eq!(doc.header().title(), Some("Document Title"));
            assert_eq!(doc.header().title_source().unwrap().line(), 1);
        }
    }
}
