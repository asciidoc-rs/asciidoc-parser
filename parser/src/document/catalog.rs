use std::collections::HashMap;

use crate::{content::FootnoteDeferred, internal::debug::DebugHashMapFrom, parser::XrefSignifier};

/// Document catalog for tracking referenceable elements.
///
/// The catalog maintains a registry of all elements that can be referenced
/// via cross-references, including anchors, sections, and bibliography entries.
/// It provides functionality for registering new references, resolving
/// reference text to IDs, and detecting duplicate IDs.
#[derive(Clone, Eq, PartialEq)]
pub struct Catalog {
    /// Primary registry mapping IDs to reference entries.
    pub(crate) refs: HashMap<String, RefEntry>,

    /// Reverse lookup cache: reftext -> ID.
    pub(crate) reftext_to_id: HashMap<String, String>,

    /// Footnotes registered (in document order) while substituting inline
    /// macros. Each entry corresponds to a `footnote:[…]` macro that *defined*
    /// a footnote; subsequent references to an existing footnote (via a
    /// repeated ID) reuse an entry rather than adding a new one.
    ///
    /// A nested document (an AsciiDoc table cell) keeps its own footnote list:
    /// footnotes defined inside a cell are *not* shared with the main document.
    pub(crate) footnotes: Vec<Footnote>,

    /// Images referenced by `image:`/`image::` macros, recorded in document
    /// order while substituting inline macros – but only when the parser was
    /// configured with
    /// [`with_catalog_assets(true)`](crate::Parser::with_catalog_assets)
    /// (Asciidoctor's `catalog_assets` API option). Empty otherwise.
    pub(crate) images: Vec<ImageReference>,

    /// Link targets referenced by `link:`/`mailto:` macros and by bare URL and
    /// email autolinks, recorded in document order while substituting inline
    /// macros – but only when the parser was configured with
    /// [`with_catalog_assets(true)`](crate::Parser::with_catalog_assets)
    /// (Asciidoctor's `catalog_assets` API option). Empty otherwise.
    ///
    /// Each entry is the final link target as it appears in the rendered `href`
    /// (e.g. `https://example.org`, `mailto:fred@example.com`), matching
    /// Asciidoctor's `catalog[:links]`.
    pub(crate) links: Vec<String>,

    /// AsciiDoc files that were included into this document, keyed by the
    /// include target relative to the outermost document with its AsciiDoc
    /// extension removed (e.g. `other-chapters` for
    /// `include::other-chapters.adoc[]`). The value records whether the file
    /// was ever included *in full*: `true` when at least one include merged the
    /// whole file, `false` when every include of it selected only a
    /// `lines`/`tag(s)` portion.
    ///
    /// The preprocessor records each include while it expands `include::`
    /// directives (before parsing); `Parser::parse_deferred` folds those into
    /// this map via [`register_include`](Self::register_include), and it
    /// survives into the document's catalog. It lets an
    /// inter-document cross reference whose target names an included file
    /// collapse to a same-document reference — the target's anchors are now
    /// part of *this* document — but only when the file was included in
    /// full, since a partial include may not have carried the referenced
    /// anchor across. See [`interpret_xref_target`](crate::content).
    pub(crate) includes: HashMap<String, bool>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl Catalog {
    pub(crate) fn new() -> Self {
        Self {
            refs: HashMap::new(),
            reftext_to_id: HashMap::new(),
            footnotes: Vec::new(),
            images: Vec::new(),
            links: Vec::new(),
            includes: HashMap::new(),
        }
    }

    /// Register a new referenceable element in the catalog.
    ///
    /// # Arguments
    /// * `id` - The unique identifier for the element
    /// * `reftext` - Optional reference text for the element
    /// * `ref_type` - Type of referenceable element
    ///
    /// # Returns
    /// * `Ok(())` if the element was successfully registered
    /// * `Err(DuplicateIdError)` if the ID is already in use
    pub(crate) fn register_ref(
        &mut self,
        id: &str,
        reftext: Option<&str>,
        ref_type: RefType,
    ) -> Result<(), DuplicateIdError> {
        if self.refs.contains_key(id) {
            return Err(DuplicateIdError(id.to_string()));
        }

        let entry = RefEntry {
            id: id.to_string(),
            reftext: reftext.map(|s| s.to_owned()),
            ref_type,
            signifier: None,
        };

        self.refs.insert(id.to_string(), entry);

        if let Some(reftext) = reftext {
            self.reftext_to_id
                .entry(reftext.to_string())
                .or_insert_with(|| id.to_string());
        }

        Ok(())
    }

    /// Generate a unique ID based on a base ID and register it in the catalog.
    ///
    /// If the base ID is not in use, it is returned as-is. Otherwise, numeric
    /// suffixes are appended until a unique ID is found. The generated ID is
    /// then registered in the catalog with the provided parameters.
    ///
    /// # Arguments
    /// * `base_id` - The base identifier to use
    /// * `reftext` - Optional reference text for the element
    /// * `ref_type` - Type of referenceable element
    ///
    /// # Returns
    /// The unique ID that was generated and registered.
    pub(crate) fn generate_and_register_unique_id(
        &mut self,
        base_id: &str,
        reftext: Option<&str>,
        ref_type: RefType,
        separator: &str,
    ) -> String {
        let unique_id = if !self.contains_id(base_id) {
            base_id.to_string()
        } else {
            let mut counter = 2;
            loop {
                let candidate = format!("{base_id}{separator}{counter}");
                if !self.contains_id(&candidate) {
                    break candidate;
                }
                counter += 1;
            }
        };

        // Register the generated unique ID.
        let entry = RefEntry {
            id: unique_id.clone(),
            reftext: reftext.map(|s| s.to_owned()),
            ref_type,
            signifier: None,
        };

        self.refs.insert(unique_id.clone(), entry);

        if let Some(reftext) = reftext {
            self.reftext_to_id
                .entry(reftext.to_string())
                .or_insert_with(|| unique_id.clone());
        }

        unique_id
    }

    /// Returns a reference entry by ID, if it exists.
    pub fn get_ref(&self, id: &str) -> Option<&RefEntry> {
        self.refs.get(id)
    }

    /// Returns `true` if an ID is already registered in the catalog.
    pub fn contains_id(&self, id: &str) -> bool {
        self.refs.contains_key(id)
    }

    /// Resolve reference text to an ID, if possible.
    pub fn resolve_id(&self, reftext: &str) -> Option<String> {
        self.reftext_to_id.get(reftext).cloned()
    }

    /// Attaches an [`XrefSignifier`] to an already-registered element, so a
    /// cross-reference to it can build `full`/`short`
    /// [`xrefstyle`](crate::parser::XrefStyle) text. A no-op if `id` is not
    /// registered.
    pub(crate) fn set_signifier(&mut self, id: &str, signifier: XrefSignifier) {
        if let Some(entry) = self.refs.get_mut(id) {
            entry.signifier = Some(signifier);
        }
    }

    /// Returns an iterator over all registered reference IDs, in an
    /// unspecified order.
    ///
    /// This lets a multi-document pipeline enumerate a document's anchors and
    /// section IDs (for example, to build a global cross-reference index)
    /// without re-walking the block tree.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.refs.keys().map(String::as_str)
    }

    /// Returns an iterator over all registered reference entries, in an
    /// unspecified order.
    ///
    /// Each item pairs an ID with its [`RefEntry`] (which also carries the
    /// entry's reftext and [`RefType`]).
    pub fn entries(&self) -> impl Iterator<Item = (&str, &RefEntry)> {
        self.refs.iter().map(|(id, entry)| (id.as_str(), entry))
    }

    /// Returns the number of registered references.
    pub fn len(&self) -> usize {
        self.refs.len()
    }

    /// Returns `true` if the catalog contains no registered references.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    /// Returns the footnotes registered in this document, in document order.
    pub fn footnotes(&self) -> &[Footnote] {
        &self.footnotes
    }

    /// Registers a newly-defined [`Footnote`].
    pub(crate) fn register_footnote(&mut self, footnote: Footnote) {
        self.footnotes.push(footnote);
    }

    /// Returns the registered footnote with the given ID, if one exists.
    pub(crate) fn footnote_with_id(&self, id: &str) -> Option<&Footnote> {
        self.footnotes.iter().find(|f| f.id.as_deref() == Some(id))
    }

    /// Returns the images referenced in this document, in document order.
    ///
    /// This list is populated only when the parser was configured with
    /// [`with_catalog_assets(true)`](crate::Parser::with_catalog_assets); it is
    /// empty otherwise.
    pub fn images(&self) -> &[ImageReference] {
        &self.images
    }

    /// Records a referenced image (an `image:`/`image::` macro target) in
    /// document order.
    pub(crate) fn register_image(&mut self, target: String, imagesdir: Option<String>) {
        self.images.push(ImageReference { target, imagesdir });
    }

    /// Returns the link targets referenced in this document, in document order.
    ///
    /// This list is populated only when the parser was configured with
    /// [`with_catalog_assets(true)`](crate::Parser::with_catalog_assets); it is
    /// empty otherwise.
    pub fn links(&self) -> &[String] {
        &self.links
    }

    /// Records a referenced link target (a `link:`/`mailto:` macro or an
    /// autolinked bare URL or email address) in document order.
    pub(crate) fn register_link(&mut self, target: String) {
        self.links.push(target);
    }

    /// Records that the AsciiDoc file named by `key` was included into this
    /// document.
    ///
    /// `key` is the include target relative to the outermost document, with its
    /// AsciiDoc extension removed (e.g. `other-chapters`). `full` is `true`
    /// when the entire file was included and `false` when only a
    /// `lines`/`tag(s)` selection of it was.
    ///
    /// A file included in full at least once is recorded as full even if it was
    /// also included partially (a full include always carries every anchor
    /// across), matching Asciidoctor.
    pub(crate) fn register_include(&mut self, key: &str, full: bool) {
        self.includes
            .entry(key.to_string())
            .and_modify(|existing| *existing |= full)
            .or_insert(full);
    }

    /// Returns `true` if the file named by `key` (an include target relative to
    /// the outermost document, without its AsciiDoc extension) was included
    /// into this document *in full* — i.e. at least one `include::`
    /// directive merged the whole file, rather than only a `lines`/`tag(s)`
    /// portion of it.
    ///
    /// Returns `false` if the file was only ever partially included, or was not
    /// included at all.
    pub fn include_is_full(&self, key: &str) -> bool {
        self.includes.get(key).copied().unwrap_or(false)
    }

    /// Returns `true` if the file named by `key` (an include target relative to
    /// the outermost document, without its AsciiDoc extension) was included
    /// into this document, whether in full or only partially.
    pub fn was_included(&self, key: &str) -> bool {
        self.includes.contains_key(key)
    }

    /// Removes and returns the current footnote list, leaving an empty list
    /// behind. Used to give a nested document (an AsciiDoc table cell) its own
    /// footnote registry so its footnotes are not shared with the enclosing
    /// document.
    pub(crate) fn take_footnotes(&mut self) -> Vec<Footnote> {
        std::mem::take(&mut self.footnotes)
    }

    /// Restores a previously-[taken](Self::take_footnotes) footnote list,
    /// discarding any footnotes registered in the meantime.
    pub(crate) fn restore_footnotes(&mut self, footnotes: Vec<Footnote>) {
        self.footnotes = footnotes;
    }
}

/// A footnote registered while substituting the inline `footnote:[…]` macro.
///
/// A footnote is defined at the location of its reference, but its text is
/// extracted to an item in the document's footnote list. The same footnote can
/// be referenced from multiple locations by assigning it an ID at the first
/// occurrence and repeating that ID (with empty text) afterward; only the
/// defining occurrence produces a `Footnote` entry.
#[derive(Clone, Eq, PartialEq)]
pub struct Footnote {
    /// The footnote's number, assigned in document order via the
    /// `footnote-number` counter. Normally a consecutive integer (`1`, `2`, …),
    /// but stored as a string because the counter honors any seed the document
    /// sets (e.g. `:footnote-number: z` yields `aa`, `ab`, … as Asciidoctor
    /// does).
    pub index: String,

    /// The optional ID assigned to this footnote (the target of the macro, e.g.
    /// `disclaimer` in `footnote:disclaimer[…]`). `None` for an anonymous
    /// footnote.
    pub id: Option<String>,

    /// The already-substituted text of the footnote. When the footnote contains
    /// cross-references, this reflects the unresolved fallback rendering until
    /// the document's references are resolved, after which it reflects the
    /// resolved links; it is always clean, user-facing text.
    pub text: String,

    /// Deferred cross-references discovered in the footnote text, awaiting
    /// resolution. `None` for the common case of a footnote with no
    /// cross-references.
    pub(crate) deferred: Option<Box<FootnoteDeferred>>,

    /// The location of this footnote's defining occurrence, as a
    /// `(byte offset, byte length)` pair into the document source, used to
    /// anchor a cross-reference warning at the footnote rather than at the
    /// whole document. The range spans the enclosing content the footnote was
    /// written in (paragraph granularity, matching how a non-footnote
    /// reference is anchored at its `Content`).
    ///
    /// `None` when the defining occurrence is not locatable in the document
    /// source: a footnote defined while substituting a privately-owned
    /// sub-source (a Markdown-style blockquote, an AsciiDoc table cell) indexes
    /// that owned source, which is not contiguous in the document, so storing
    /// its offset would misplace the warning. Resolution falls back to the
    /// whole-document span in that case.
    pub(crate) location: Option<(usize, usize)>,
}

impl Footnote {
    /// Resolves any cross-references embedded in this footnote's text using
    /// `resolver`, then rebuilds [`text`](Self::text) from the resolved state.
    /// Any unresolved target is reported in `warnings`.
    ///
    /// A footnote's text is extracted out of the block it was defined in, so
    /// the warning is anchored using the footnote's recorded
    /// [`location`](Self::location) — the enclosing content it was written in —
    /// reconstructed as a sub-span of `document_source`. When no location was
    /// recorded (a footnote defined inside an owned sub-source, whose offset
    /// does not map to the document), the warning falls back to the whole
    /// `document_source` span.
    ///
    /// A footnote with no cross-references is left untouched.
    pub(crate) fn resolve_references<'src>(
        &mut self,
        resolver: &dyn crate::parser::ReferenceResolver,
        renderer: &dyn crate::parser::InlineSubstitutionRenderer,
        warnings: &mut crate::parser::ReferenceWarnings<'src>,
        document_source: crate::Span<'src>,
    ) {
        if let Some(deferred) = self.deferred.as_mut() {
            let source = match self.location {
                Some((offset, len)) => document_source.slice(offset..offset + len),
                None => document_source,
            };
            deferred.resolve(resolver, warnings, source);
            self.text = deferred.render(renderer);
        }
    }
}

impl std::fmt::Debug for Footnote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The deferred cross-reference state is an internal implementation
        // detail, omitted unless present so that the (very common)
        // cross-reference-free footnote debugs as a plain field set.
        let mut s = f.debug_struct("Footnote");
        s.field("index", &self.index);
        s.field("id", &self.id);
        s.field("text", &self.text);

        if let Some(deferred) = self.deferred.as_ref() {
            s.field("deferred", deferred);
        }

        s.finish()
    }
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("refs", &DebugHashMapFrom(&self.refs))
            .field("reftext_to_id", &DebugHashMapFrom(&self.reftext_to_id))
            .field("footnotes", &self.footnotes)
            .field("images", &self.images)
            .field("links", &self.links)
            .field("includes", &DebugHashMapFrom(&self.includes))
            .finish()
    }
}

/// A reference to an image asset recorded in the document
/// [`Catalog`](Catalog::images) when `catalog_assets` is enabled.
///
/// Mirrors Asciidoctor's `Document::ImageReference`: it pairs the image
/// [`target`](Self::target) with the value of the `imagesdir` attribute in
/// effect where the image was referenced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageReference {
    /// The image target as written in the macro, after attribute references in
    /// the target have been substituted (e.g. `fixtures/dot.gif`).
    pub target: String,

    /// The value of the `imagesdir` document attribute at the point of
    /// reference, or `None` when it was unset.
    pub imagesdir: Option<String>,
}

impl std::fmt::Display for ImageReference {
    /// Displays the image reference as its [`target`](Self::target), mirroring
    /// Asciidoctor's `ImageReference#to_s`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.target)
    }
}

/// Type of referenceable element in the document.
#[derive(Clone, PartialEq, Eq)]
pub enum RefType {
    /// Standard anchor element (`[[id]]` or `[[id,reftext]]`).
    Anchor,

    /// Section heading that can be referenced.
    Section,

    /// Bibliography reference (`[[[id]]]` or `[[[id,reftext]]]`).
    Bibliography,
}

impl std::fmt::Debug for RefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anchor => f.write_str("RefType::Anchor"),
            Self::Section => f.write_str("RefType::Section"),
            Self::Bibliography => f.write_str("RefType::Bibliography"),
        }
    }
}

/// Entry in the document catalog representing a referenceable element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefEntry {
    /// The unique identifier for this element.
    pub id: String,

    /// Reference text for this element (explicit or computed).
    pub reftext: Option<String>,

    /// Type of referenceable element.
    pub ref_type: RefType,

    /// The signifier and number used to build `full`/`short`
    /// [`xrefstyle`](crate::parser::XrefStyle) cross-reference text for this
    /// target. Present only for a numbered section or captioned block that has
    /// no explicit reftext; `None` for every other element (plain anchors,
    /// bibliography entries, unnumbered sections, and targets carrying an
    /// explicit reftext, for which `xrefstyle` formatting does not apply).
    pub signifier: Option<XrefSignifier>,
}

/// Error that occurs when attempting to register a duplicate ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateIdError(pub(crate) String);

impl std::fmt::Display for DuplicateIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID '{}' already registered", self.0)
    }
}

impl std::error::Error for DuplicateIdError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn new_catalog_is_empty() {
        let catalog = Catalog::new();
        assert!(catalog.is_empty());
        assert_eq!(catalog.len(), 0);
    }

    #[test]
    fn register_ref_success() {
        let mut catalog = Catalog::new();

        let result = catalog.register_ref("test-id", Some("Test Reference"), RefType::Anchor);

        assert!(result.is_ok());
        assert_eq!(catalog.len(), 1);
        assert!(catalog.contains_id("test-id"));
    }

    #[test]
    fn register_duplicate_id_fails() {
        let mut catalog = Catalog::new();

        // Register first reference.
        catalog
            .register_ref("test-id", Some("First"), RefType::Anchor)
            .unwrap();

        // Attempt to register duplicate.
        let result = catalog.register_ref("test-id", Some("Second"), RefType::Section);

        let error = result.unwrap_err();
        assert_eq!(error.0, "test-id");
    }

    #[test]
    fn generate_and_register_unique_id() {
        let mut catalog = Catalog::new();

        // Test with available ID.
        let id1 = catalog.generate_and_register_unique_id(
            "available",
            Some("Available Ref"),
            RefType::Anchor,
            "-",
        );
        assert_eq!(id1, "available");
        assert!(catalog.contains_id("available"));
        assert_eq!(
            catalog.resolve_id("Available Ref"),
            Some("available".to_string())
        );

        // Test with taken IDs.
        catalog
            .register_ref("taken", None, RefType::Anchor)
            .unwrap();
        catalog
            .register_ref("taken-2", None, RefType::Anchor)
            .unwrap();

        let id2 = catalog.generate_and_register_unique_id("taken", None, RefType::Section, "-");
        assert_eq!(id2, "taken-3");
        assert!(catalog.contains_id("taken-3"));
    }

    #[test]
    fn get_ref() {
        let mut catalog = Catalog::new();

        catalog
            .register_ref("test-id", Some("Test Reference"), RefType::Bibliography)
            .unwrap();

        let entry = catalog.get_ref("test-id").unwrap();
        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.reftext, Some("Test Reference".to_string()));
        assert_eq!(entry.ref_type, RefType::Bibliography);

        assert!(catalog.get_ref("nonexistent").is_none());
    }

    #[test]
    fn enumerate_ids_and_entries() {
        let mut catalog = Catalog::new();

        catalog
            .register_ref("intro", Some("Introduction"), RefType::Section)
            .unwrap();
        catalog
            .register_ref("fig-1", None, RefType::Anchor)
            .unwrap();

        // `ids()` enumerates every registered ID (order is unspecified).
        let mut ids: Vec<&str> = catalog.ids().collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["fig-1", "intro"]);

        // `entries()` pairs each ID with its full entry.
        let entries: Vec<(&str, &RefEntry)> = catalog.entries().collect();
        assert_eq!(entries.len(), 2);

        let (fig_id, fig_entry) = entries.iter().find(|(id, _)| *id == "fig-1").unwrap();
        assert_eq!(*fig_id, "fig-1");
        assert_eq!(fig_entry.id, "fig-1");
        assert_eq!(fig_entry.reftext, None);
        assert_eq!(fig_entry.ref_type, RefType::Anchor);

        let (_, intro_entry) = entries.iter().find(|(id, _)| *id == "intro").unwrap();
        assert_eq!(intro_entry.reftext, Some("Introduction".to_string()));
        assert_eq!(intro_entry.ref_type, RefType::Section);
    }

    #[test]
    fn resolve_id() {
        let mut catalog = Catalog::new();

        catalog
            .register_ref("anchor1", Some("Reference Text"), RefType::Anchor)
            .unwrap();

        catalog
            .register_ref("anchor2", Some("Another Reference"), RefType::Section)
            .unwrap();

        assert_eq!(
            catalog.resolve_id("Reference Text"),
            Some("anchor1".to_string())
        );
        assert_eq!(
            catalog.resolve_id("Another Reference"),
            Some("anchor2".to_string())
        );
        assert_eq!(catalog.resolve_id("Nonexistent"), None);
    }

    #[test]
    fn resolve_id_first_wins_on_duplicates() {
        let mut catalog = Catalog::new();

        // Register two different IDs with same reftext.
        catalog
            .register_ref("first", Some("Same Text"), RefType::Anchor)
            .unwrap();

        catalog
            .register_ref("second", Some("Same Text"), RefType::Section)
            .unwrap();

        assert_eq!(catalog.resolve_id("Same Text"), Some("first".to_string()));
    }

    #[test]
    fn register_include_records_full_and_partial() {
        let mut catalog = Catalog::new();

        // An unregistered file is neither included nor full.
        assert!(!catalog.was_included("tigers"));
        assert!(!catalog.include_is_full("tigers"));

        catalog.register_include("tigers", false);
        assert!(catalog.was_included("tigers"));
        assert!(!catalog.include_is_full("tigers"));

        catalog.register_include("lions", true);
        assert!(catalog.was_included("lions"));
        assert!(catalog.include_is_full("lions"));
    }

    #[test]
    fn a_full_include_wins_over_a_partial_one_in_either_order() {
        // partial then full → full
        let mut catalog = Catalog::new();
        catalog.register_include("tigers", false);
        catalog.register_include("tigers", true);
        assert!(catalog.include_is_full("tigers"));

        // full then partial → still full
        let mut catalog = Catalog::new();
        catalog.register_include("tigers", true);
        catalog.register_include("tigers", false);
        assert!(catalog.include_is_full("tigers"));

        // partial then partial → partial
        let mut catalog = Catalog::new();
        catalog.register_include("tigers", false);
        catalog.register_include("tigers", false);
        assert!(catalog.was_included("tigers"));
        assert!(!catalog.include_is_full("tigers"));
    }

    #[test]
    fn register_image_records_in_document_order() {
        let mut catalog = Catalog::new();
        assert!(catalog.images().is_empty());

        catalog.register_image("fixtures/dot.gif".to_string(), None);
        catalog.register_image("logo.png".to_string(), Some("images".to_string()));

        let images = catalog.images();
        assert_eq!(images.len(), 2);

        // The first image carries no `imagesdir`; `to_string`/`Display` yields
        // the bare target.
        assert_eq!(images[0].target, "fixtures/dot.gif");
        assert_eq!(images[0].imagesdir, None);
        assert_eq!(images[0].to_string(), "fixtures/dot.gif");

        // The second records the `imagesdir` in effect at the reference.
        assert_eq!(images[1].target, "logo.png");
        assert_eq!(images[1].imagesdir, Some("images".to_string()));
        assert_eq!(images[1].to_string(), "logo.png");
    }

    #[test]
    fn register_link_records_in_document_order() {
        let mut catalog = Catalog::new();
        assert!(catalog.links().is_empty());

        catalog.register_link("https://example.org".to_string());
        catalog.register_link("mailto:fred@example.com".to_string());

        assert_eq!(
            catalog.links(),
            ["https://example.org", "mailto:fred@example.com"]
        );
    }

    #[test]
    fn duplicate_id_error_impl_display() {
        let did_error = DuplicateIdError("foo".to_string());
        assert_eq!(did_error.to_string(), "ID 'foo' already registered");
    }

    #[test]
    fn ref_type_impl_debug() {
        assert_eq!(format!("{:#?}", RefType::Anchor), "RefType::Anchor");
        assert_eq!(format!("{:#?}", RefType::Section), "RefType::Section");

        assert_eq!(
            format!("{:#?}", RefType::Bibliography),
            "RefType::Bibliography"
        );
    }
}
