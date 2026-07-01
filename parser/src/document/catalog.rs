use std::collections::HashMap;

use crate::internal::debug::DebugHashMapFrom;

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
    ) -> String {
        let unique_id = if !self.contains_id(base_id) {
            base_id.to_string()
        } else {
            let mut counter = 2;
            loop {
                let candidate = format!("{}-{}", base_id, counter);
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

    /* Disabling for now until I know if we'll need these.

    /// Returns an iterator over all registered reference IDs.
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.refs.keys()
    }

    /// Returns an iterator over all reference entries.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &RefEntry)> {
        self.refs.iter()
    }
    */

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
#[derive(Clone, Debug, Eq, PartialEq)]
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

    /// The already-substituted text of the footnote.
    pub text: String,
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("refs", &DebugHashMapFrom(&self.refs))
            .field("reftext_to_id", &DebugHashMapFrom(&self.reftext_to_id))
            .field("footnotes", &self.footnotes)
            .finish()
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
    #![allow(clippy::unwrap_used)]

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

        let id2 = catalog.generate_and_register_unique_id("taken", None, RefType::Section);
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
