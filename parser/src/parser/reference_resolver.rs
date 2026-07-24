//! Cross-reference resolution.
//!
//! Parsing leaves cross-references (`<<id>>`, `xref:id[…]`) unresolved so they
//! can be resolved later, once the full document – or, for multi-document
//! workflows such as Antora, the full corpus – has been parsed and its catalog
//! of referenceable elements is complete.
//!
//! Resolution is performed through the [`ReferenceResolver`] trait. This crate
//! ships [`CatalogResolver`], a single-document resolver backed by one
//! [`Catalog`]. A host that resolves references across many documents supplies
//! its own implementation (binding the "from" document when it constructs the
//! resolver), and this crate makes no attempt to merge catalogs.

use crate::{
    Span,
    document::Catalog,
    warnings::{Warning, WarningType},
};

/// The cross-reference text style selected by the `xrefstyle` attribute.
///
/// The style is chosen from the `xrefstyle` value in effect for a reference:
/// the `xrefstyle=` attribute on the `xref:` macro if present, otherwise the
/// document-wide `xrefstyle` attribute. It controls how the automatic text of a
/// cross-reference is generated for a target that carries a reference number
/// (see [Cross reference styles]).
///
/// A reference whose `xrefstyle` is *unset* has no `XrefStyle`; it uses the
/// target's reference text verbatim. An unrecognized value is treated as
/// [`Basic`](Self::Basic), mirroring Asciidoctor.
///
/// [Cross reference styles]: https://docs.asciidoctor.org/asciidoc/latest/macros/xref-text-and-style/#cross-reference-styles
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XrefStyle {
    /// The signifier and number followed by the title, quoted (or emphasized
    /// for a chapter or appendix): e.g. `Section 2.3, “Installation”`.
    Full,

    /// The signifier and number only: e.g. `Section 2.3`.
    Short,

    /// The title only, emphasized for a chapter or appendix: e.g.
    /// `Installation`.
    Basic,
}

impl XrefStyle {
    /// Interprets an `xrefstyle` attribute value. `full` and `short` select
    /// those styles; every other value (including `basic` and any unrecognized
    /// value) yields [`Basic`](Self::Basic), mirroring Asciidoctor.
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "full" => Self::Full,
            "short" => Self::Short,
            _ => Self::Basic,
        }
    }
}

/// A referenceable target's signifier and reference number, used to build the
/// automatic text of a cross-reference for the [`Full`](XrefStyle::Full) and
/// [`Short`](XrefStyle::Short) styles (and to emphasize a chapter or appendix
/// title under [`Basic`](XrefStyle::Basic)).
///
/// This carries only the target-derived pieces; how they are combined with the
/// target's title is decided by the reference's [`XrefStyle`] at render time.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct XrefSignifier {
    /// The signifier and reference number, already combined (e.g. `"Section
    /// 2.3"`, `"Figure 1"`, or just `"2.3"` when the target's `*-refsig`
    /// attribute is unset).
    pub label: String,

    /// Whether the target's title is emphasized (rendered inside `<em>`) rather
    /// than quoted. `true` for chapters and appendices.
    pub emphasize: bool,
}

/// The resolved destination of a cross-reference.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedReference {
    /// The hyperlink destination. For a same-document reference this is a
    /// fragment such as `#section-id`; a cross-document resolver may return a
    /// full or relative URL.
    pub href: String,

    /// The display text to use when the cross-reference did not specify its own
    /// text. This is typically the target's reference text (reftext).
    pub text: Option<String>,

    /// The target's signifier and number, when it carries one and has no
    /// explicit reftext. Present only for targets eligible for `full`/`short`
    /// [`xrefstyle`](XrefStyle) formatting (numbered sections and captioned
    /// blocks); `None` otherwise. Ignored unless the reference selects a style.
    pub signifier: Option<XrefSignifier>,
}

impl ResolvedReference {
    /// Constructs a resolved reference with no [`signifier`](Self::signifier).
    ///
    /// Use this when the target is not a numbered/captioned element, or when
    /// the resolver builds the display `text` from scratch. When the target
    /// came from a [`Catalog`] (the usual case, including cross-document
    /// resolution), prefer [`from_entry`](Self::from_entry) so
    /// `full`/`short` `xrefstyle` formatting keeps working; or attach a
    /// signifier explicitly with [`with_signifier`](Self::with_signifier).
    pub fn new(href: String, text: Option<String>) -> Self {
        Self {
            href,
            text,
            signifier: None,
        }
    }

    /// Constructs a resolved reference to a catalog element at `href`, carrying
    /// the element's reference text **and** its
    /// [`signifier`](Self::signifier).
    ///
    /// This is the seam that makes `full`/`short` `xrefstyle` formatting work
    /// across documents: a multi-document (Antora-style) resolver that has
    /// located the target's [`RefEntry`] in some document's [`Catalog`] passes
    /// the `href` it computed for that document, and the target's signifier and
    /// number – computed while *that* document was parsed – ride along. The
    /// style itself comes from the *referencing* document and is applied later,
    /// so the resolver need not know it. The single-document
    /// [`CatalogResolver`] is built on this same helper.
    ///
    /// [`RefEntry`]: crate::document::RefEntry
    pub fn from_entry(href: String, entry: &crate::document::RefEntry) -> Self {
        Self {
            href,
            text: entry.reftext.clone(),
            signifier: entry.signifier.clone(),
        }
    }

    /// Attaches a [`signifier`](Self::signifier), returning `self` for
    /// chaining.
    ///
    /// For a host resolver that builds its `href`/`text` from scratch but still
    /// wants `full`/`short` `xrefstyle` formatting for a numbered or captioned
    /// target.
    pub fn with_signifier(mut self, signifier: XrefSignifier) -> Self {
        self.signifier = Some(signifier);
        self
    }
}

/// The destination a cross-reference target resolves to on its own, without
/// consulting any catalog.
///
/// A target that names a document – another one (an [inter-document cross
/// reference]) or the current one – carries its own destination. The parser
/// derives it while substituting the reference, rewriting the path with the
/// `relfileprefix`, `relfilesuffix`, and `outfilesuffix` attributes in effect
/// at that point in the document.
///
/// This is a *default*: a [`ReferenceResolver`] that knows better (an
/// Antora-style host that resolves targets across a corpus) may still return
/// its own [`ResolvedReference`], which takes precedence. This is what is used
/// when it does not.
///
/// [inter-document cross reference]: https://docs.asciidoctor.org/asciidoc/latest/macros/inter-document-xref/
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DerivedReference {
    /// The hyperlink destination: the rewritten output path plus the target's
    /// fragment, if it had one (e.g. `tigers.html#about`), or `#` for a
    /// reference to the current document.
    pub href: String,

    /// The display text to use when the cross-reference did not supply its
    /// own.
    ///
    /// For another document this is its output path (e.g. `tigers.html`),
    /// since that document's reference text is not available to a
    /// single-document parse. For the current document it is the document's
    /// `reftext` or, failing that, its title.
    pub text: String,
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

/// Accumulates what a cross-reference resolution sweep found, in the two forms
/// the crate needs to report it.
///
/// Both lists describe the same conditions: [`host`](Self::host) is handed back
/// to whoever drove the sweep (and is the crate's public resolution API), while
/// [`doc`](Self::doc) is folded into the document's own
/// [warnings](crate::Document::warnings) so an unresolved reference shows up
/// alongside every other parse-time diagnostic.
#[derive(Default)]
pub(crate) struct ReferenceWarnings<'src> {
    /// The warnings returned from the resolution pass.
    pub(crate) host: Vec<ReferenceWarning>,

    /// The same warnings, anchored to the source they were found in.
    pub(crate) doc: Vec<Warning<'src>>,
}

impl<'src> ReferenceWarnings<'src> {
    /// Records a target that `resolver` could not resolve, found within
    /// `source`.
    pub(crate) fn unresolved(&mut self, target: &str, source: Span<'src>) {
        self.host.push(ReferenceWarning {
            target: target.to_string(),
            kind: ReferenceWarningKind::Unresolved,
        });

        self.doc.push(Warning {
            source,
            warning: WarningType::PossibleInvalidReference(target.to_string()),
            origin: None,
        });
    }

    /// Folds warnings gathered from a privately-owned sub-parse – the blocks of
    /// a Markdown-style blockquote, or of an include-expanded AsciiDoc table
    /// cell – into `dest`.
    ///
    /// Those blocks borrow their own owned source, so their spans cannot be
    /// named in the enclosing document. Each document warning is re-anchored to
    /// `anchor`, the enclosing element's span in the document.
    pub(crate) fn rehome_into<'outer>(
        self,
        dest: &mut ReferenceWarnings<'outer>,
        anchor: Span<'outer>,
    ) {
        dest.host.extend(self.host);

        dest.doc.extend(self.doc.into_iter().map(|warning| Warning {
            source: anchor,
            warning: warning.warning,
            origin: warning.origin,
        }));
    }
}

/// Describes a single cross-reference that needs to be resolved.
///
/// This carries only information the crate itself knows about the reference. A
/// multi-document host that needs to know which document a reference originates
/// from binds that "from" context when it constructs its [`ReferenceResolver`],
/// rather than receiving it here – keeping this seam free of any host-specific
/// coordinate system.
#[non_exhaustive]
pub struct ResolutionContext<'a> {
    /// The raw, uninterpreted cross-reference target, exactly as written in the
    /// source (e.g. `"section-id"`, a reftext, or `"other-page.adoc#frag"`).
    pub target: &'a str,

    /// Explicit link text supplied in the cross-reference, if any.
    pub provided_text: Option<&'a str>,

    /// The destination the parser derived from the target itself, for a
    /// target that names a document; `None` for a reference to an element
    /// within the current document.
    ///
    /// A resolver that can do better is free to ignore this and return its own
    /// [`ResolvedReference`]; returning `None` leaves this default in place.
    pub derived: Option<&'a DerivedReference>,
}

impl<'a> ResolutionContext<'a> {
    /// Constructs a [`ResolutionContext`] from its parts.
    ///
    /// The crate itself builds these values internally; this constructor exists
    /// so a downstream [`ReferenceResolver`] implementation can build its own
    /// contexts in unit tests despite the type being `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        target: &'a str,
        provided_text: Option<&'a str>,
        derived: Option<&'a DerivedReference>,
    ) -> Self {
        Self {
            target,
            provided_text,
            derived,
        }
    }
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
/// `#id` fragments. A target that names a document (e.g.
/// `other-page.adoc#frag`) is left unresolved here, so it falls back to the
/// [`DerivedReference`] the parser built from the target's path; only a
/// host-supplied resolver, which can see the other document, can do better.
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

        // A target that names a document already carries its destination.
        if context.derived.is_some() {
            return None;
        }

        // Direct ID match.
        if let Some(entry) = self.catalog.get_ref(target) {
            return Some(ResolvedReference::from_entry(format!("#{target}"), entry));
        }

        // Natural cross-reference: match on reference text.
        if let Some(id) = self.catalog.resolve_id(target) {
            return self
                .catalog
                .get_ref(&id)
                .map(|entry| ResolvedReference::from_entry(format!("#{id}"), entry));
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
                derived: None,
            })
            .unwrap();

        assert_eq!(resolved.href, "#later");
        assert_eq!(resolved.text.as_deref(), Some("The Later Section"));
    }

    #[test]
    fn new_builds_a_resolvable_context() {
        // The `new` constructor is the seam a downstream `ReferenceResolver`
        // uses to build its own contexts, since the type is `#[non_exhaustive]`.
        let catalog = catalog_with("later", Some("The Later Section"), RefType::Section);
        let resolver = CatalogResolver::new(&catalog);

        let context = ResolutionContext::new("later", None, None);
        assert_eq!(context.target, "later");
        assert_eq!(context.provided_text, None);
        assert!(context.derived.is_none());

        let resolved = resolver.resolve(&context).unwrap();
        assert_eq!(resolved.href, "#later");
    }

    #[test]
    fn resolves_by_reftext() {
        let catalog = catalog_with("later", Some("The Later Section"), RefType::Section);
        let resolver = CatalogResolver::new(&catalog);

        let resolved = resolver
            .resolve(&ResolutionContext {
                target: "The Later Section",
                provided_text: None,
                derived: None,
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
                    derived: None,
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
                    derived: Some(&DerivedReference {
                        href: "other-page.html#frag".to_string(),
                        text: "other-page.html".to_string(),
                    }),
                })
                .is_none()
        );
    }

    #[test]
    fn numeric_character_reference_is_not_a_path_separator() {
        let catalog = catalog_with("_cub_tiger", Some("Cub &#8658; Tiger"), RefType::Section);
        let resolver = CatalogResolver::new(&catalog);

        let resolved = resolver
            .resolve(&ResolutionContext {
                target: "Cub &#8658; Tiger",
                provided_text: None,
                derived: None,
            })
            .unwrap();

        assert_eq!(resolved.href, "#_cub_tiger");
        assert_eq!(resolved.text.as_deref(), Some("Cub &#8658; Tiger"));
    }
}
