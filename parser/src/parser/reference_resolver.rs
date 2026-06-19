//! Cross-reference resolution.
//!
//! Parsing leaves cross-references (`<<id>>`, `xref:id[…]`) unresolved so they
//! can be resolved later, once the full document — or, for multi-document
//! workflows such as Antora, the full corpus — has been parsed and its catalog
//! of referenceable elements is complete.
//!
//! Resolution is performed through the [`ReferenceResolver`] trait. This crate
//! ships [`CatalogResolver`], a single-document resolver backed by one
//! [`Catalog`]. A host that resolves references across many documents supplies
//! its own implementation (binding the "from" document when it constructs the
//! resolver), and this crate makes no attempt to merge catalogs.

use crate::document::Catalog;

/// The resolved destination of a cross-reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedReference {
    /// The hyperlink destination. For a same-document reference this is a
    /// fragment such as `#section-id`; a cross-document resolver may return a
    /// full or relative URL.
    pub href: String,

    /// The display text to use when the cross-reference did not specify its own
    /// text. This is typically the target's reference text (reftext).
    pub text: Option<String>,
}

/// A warning produced while resolving cross-references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceWarning {
    /// The cross-reference target that could not be resolved, exactly as
    /// written in the source.
    pub target: String,

    /// The kind of problem encountered.
    pub kind: ReferenceWarningKind,
}

/// The kind of problem described by a [`ReferenceWarning`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReferenceWarningKind {
    /// The target could not be resolved to any destination.
    Unresolved,
}

/// Describes a single cross-reference that needs to be resolved.
///
/// This carries only information the crate itself knows about the reference. A
/// multi-document host that needs to know which document a reference originates
/// from binds that "from" context when it constructs its [`ReferenceResolver`],
/// rather than receiving it here — keeping this seam free of any host-specific
/// coordinate system.
#[non_exhaustive]
pub struct ResolutionContext<'a> {
    /// The raw, uninterpreted cross-reference target, exactly as written in the
    /// source (e.g. `"section-id"`, a reftext, or `"other-page.adoc#frag"`).
    pub target: &'a str,

    /// Explicit link text supplied in the cross-reference, if any.
    pub provided_text: Option<&'a str>,
}

/// Resolves cross-reference targets to their destinations.
///
/// Implementations map a [`ResolutionContext`] to a [`ResolvedReference`], or
/// return `None` when the target cannot be resolved (the caller then renders an
/// unresolved-reference fallback and may emit a warning).
pub trait ReferenceResolver {
    /// Resolve a single cross-reference.
    fn resolve(&self, context: &ResolutionContext<'_>) -> Option<ResolvedReference>;
}

/// The default single-document [`ReferenceResolver`], backed by one
/// [`Catalog`].
///
/// It resolves bare IDs and natural cross-references (by reference text) to
/// `#id` fragments. Targets that carry a path component (detected by the
/// presence of `#`, e.g. `other-page.adoc#frag`) are treated as inter-document
/// references and left unresolved — resolving those is the responsibility of a
/// host-supplied resolver.
#[derive(Clone, Copy, Debug)]
pub struct CatalogResolver<'a> {
    catalog: &'a Catalog,
}

impl<'a> CatalogResolver<'a> {
    /// Construct a resolver backed by the given catalog.
    pub fn new(catalog: &'a Catalog) -> Self {
        Self { catalog }
    }
}

impl ReferenceResolver for CatalogResolver<'_> {
    fn resolve(&self, context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
        let target = context.target;

        // Path-bearing (inter-document) targets are a host concern.
        if target.contains('#') {
            return None;
        }

        // Direct ID match.
        if let Some(entry) = self.catalog.get_ref(target) {
            return Some(ResolvedReference {
                href: format!("#{target}"),
                text: entry.reftext.clone(),
            });
        }

        // Natural cross-reference: match on reference text.
        if let Some(id) = self.catalog.resolve_id(target) {
            let text = self.catalog.get_ref(&id).and_then(|e| e.reftext.clone());
            return Some(ResolvedReference {
                href: format!("#{id}"),
                text,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::document::RefType;

    fn catalog_with(id: &str, reftext: Option<&str>, ref_type: RefType) -> Catalog {
        let mut catalog = Catalog::new();
        catalog.register_ref(id, reftext, ref_type).unwrap();
        catalog
    }

    #[test]
    fn resolves_by_id() {
        let catalog = catalog_with("later", Some("The Later Section"), RefType::Section);
        let resolver = CatalogResolver::new(&catalog);

        let resolved = resolver
            .resolve(&ResolutionContext {
                target: "later",
                provided_text: None,
            })
            .unwrap();

        assert_eq!(resolved.href, "#later");
        assert_eq!(resolved.text.as_deref(), Some("The Later Section"));
    }

    #[test]
    fn resolves_by_reftext() {
        let catalog = catalog_with("later", Some("The Later Section"), RefType::Section);
        let resolver = CatalogResolver::new(&catalog);

        let resolved = resolver
            .resolve(&ResolutionContext {
                target: "The Later Section",
                provided_text: None,
            })
            .unwrap();

        assert_eq!(resolved.href, "#later");
        assert_eq!(resolved.text.as_deref(), Some("The Later Section"));
    }

    #[test]
    fn unresolved_returns_none() {
        let catalog = Catalog::new();
        let resolver = CatalogResolver::new(&catalog);

        assert!(
            resolver
                .resolve(&ResolutionContext {
                    target: "missing",
                    provided_text: None,
                })
                .is_none()
        );
    }

    #[test]
    fn path_bearing_target_left_unresolved() {
        let catalog = catalog_with("frag", Some("Fragment"), RefType::Anchor);
        let resolver = CatalogResolver::new(&catalog);

        assert!(
            resolver
                .resolve(&ResolutionContext {
                    target: "other-page.adoc#frag",
                    provided_text: None,
                })
                .is_none()
        );
    }
}
