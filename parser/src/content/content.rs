//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use crate::{
    Span,
    parser::{
        InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarning, ReferenceWarningKind,
        ResolutionContext, XrefRenderParams,
    },
    strings::CowStr,
};

/// Describes the annotated content of a block after any relevant
/// [substitutions] have been performed.
///
/// This is typically used to represent the main body of block types that don't
/// contain other blocks, such as [`SimpleBlock`] or [`RawDelimitedBlock`].
///
/// # Deferred cross-references
///
/// Cross-references (`<<id>>`, `xref:id[…]`) cannot be resolved while a block
/// is being parsed, because their target may be defined later in the document
/// (or, for multi-document workflows, in another document entirely). The
/// macros substitution therefore records each cross-reference in a deferred
/// form and leaves an opaque placeholder in the rendered text. The
/// references are resolved in a later pass — see
/// [`Document::resolve_references`] — at which point [`rendered()`] reflects
/// the resolved links. Until then, [`rendered()`] shows an unresolved fallback,
/// so it always returns clean text.
///
/// [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/
/// [`SimpleBlock`]: crate::blocks::SimpleBlock
/// [`RawDelimitedBlock`]: crate::blocks::RawDelimitedBlock
/// [`Document::resolve_references`]: crate::Document::resolve_references
/// [`rendered()`]: Self::rendered
#[derive(Clone, Eq, PartialEq)]
pub struct Content<'src> {
    /// The original [`Span`] from which this content was derived.
    original: Span<'src>,

    /// The possibly-modified text after substititions have been performed.
    ///
    /// This is always clean, user-facing text: when cross-references are still
    /// unresolved it holds the unresolved fallback rendering, and after
    /// resolution it holds the resolved rendering.
    pub(crate) rendered: CowStr<'src>,

    /// Deferred cross-references discovered during substitution, awaiting
    /// resolution against a (possibly cross-document) catalog.
    ///
    /// `None` for the overwhelming majority of content, which contains no
    /// cross-references.
    deferred: Option<Box<DeferredContent>>,
}

/// The deferred (cross-reference-bearing) portion of a [`Content`].
#[derive(Clone, Debug, Eq, PartialEq)]
struct DeferredContent {
    /// The locally-substituted text with opaque placeholder tokens marking
    /// where each cross-reference will be spliced in. This is the source of
    /// truth from which [`Content::rendered`] is (re)built, so resolution is
    /// non-destructive and may be repeated.
    template: String,

    /// The cross-references, in placeholder order.
    xrefs: Vec<XrefSegment>,
}

/// A single deferred cross-reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XrefSegment {
    /// The raw, uninterpreted target as written in the source.
    pub(crate) target: String,

    /// Explicit link text supplied in the cross-reference, if any.
    pub(crate) provided_text: Option<String>,

    /// The resolved destination, filled in by resolution; `None` until then.
    pub(crate) resolved: Option<crate::parser::ResolvedReference>,
}

/// Sentinel codepoints (Unicode Private Use Area) bracketing a placeholder
/// index in [`DeferredContent::template`]. These cannot collide with user text
/// and are inert to the remaining substitution steps.
const XREF_PLACEHOLDER_START: char = '\u{E000}';
const XREF_PLACEHOLDER_END: char = '\u{E001}';

impl<'src> Content<'src> {
    /// Constructs a `Content` from a source `Span` and a potentially-filtered
    /// view of that source text.
    pub(crate) fn from_filtered<T: AsRef<str>>(span: Span<'src>, filtered: T) -> Self {
        Self {
            original: span,
            rendered: filtered.as_ref().to_string().into(),
            deferred: None,
        }
    }

    /// Returns the original span from which this [`Content`] was derived.
    ///
    /// This is the source text before any substitions have been applied.
    pub fn original(&self) -> Span<'src> {
        self.original
    }

    /// Returns the final text after all substitutions have been applied.
    pub fn rendered(&'src self) -> &'src str {
        self.rendered.as_ref()
    }

    /// Returns an owned copy of the final text after all substitutions have
    /// been applied.
    ///
    /// Unlike [`rendered()`](Self::rendered), this does not tie the returned
    /// value to the `'src` lifetime, so it can be called on a short-lived
    /// `Content` built solely to render a fragment (e.g. a block's attribution
    /// or citation text).
    pub(crate) fn rendered_owned(&self) -> String {
        self.rendered.as_ref().to_string()
    }

    /// Returns `true` if `self` contains no text.
    pub fn is_empty(&self) -> bool {
        self.rendered.as_ref().is_empty()
    }

    /// Returns `true` if this content contains one or more cross-references
    /// that have not yet been resolved to a destination.
    pub fn has_unresolved_refs(&self) -> bool {
        self.deferred
            .as_ref()
            .is_some_and(|d| d.xrefs.iter().any(|x| x.resolved.is_none()))
    }

    /// Records the cross-references discovered for this content during the
    /// macros substitution step. The placeholder tokens for these references
    /// must already have been written into [`Content::rendered`], in the same
    /// order as `xrefs`.
    ///
    /// This must be called at most once per `Content`: the placeholder indices
    /// already embedded in [`Content::rendered`] are positions into this single
    /// `xrefs` vector. The macros substitution runs once per content, so this
    /// holds in practice; the assertion guards against a future caller breaking
    /// it.
    pub(crate) fn set_deferred_xrefs(&mut self, xrefs: Vec<XrefSegment>) {
        if xrefs.is_empty() {
            return;
        }

        debug_assert!(
            self.deferred.is_none(),
            "set_deferred_xrefs must be called at most once per Content"
        );

        self.deferred = Some(Box::new(DeferredContent {
            template: String::new(),
            xrefs,
        }));
    }

    /// Returns the placeholder token for the cross-reference at `index`.
    pub(crate) fn xref_placeholder(index: usize) -> String {
        format!("{XREF_PLACEHOLDER_START}{index}{XREF_PLACEHOLDER_END}")
    }

    /// Finalizes any deferred cross-references at the end of substitution.
    ///
    /// At this point [`Content::rendered`] holds the placeholder-bearing text;
    /// it is captured as the template and `rendered` is rebuilt as the
    /// unresolved fallback so it is immediately clean for callers that read it
    /// before resolution.
    pub(crate) fn finalize_deferred(&mut self, renderer: &dyn InlineSubstitutionRenderer) {
        if self.deferred.is_none() {
            return;
        }

        let template = self.rendered.as_ref().to_string();

        if let Some(deferred) = self.deferred.as_mut() {
            deferred.template = template;
        }

        self.rebuild_rendered(renderer);
    }

    /// Resolves any deferred cross-references using `resolver`, then rebuilds
    /// the rendered text.
    ///
    /// This is non-destructive: the placeholder template is retained, so a
    /// document may be resolved more than once (e.g. for incremental builds or
    /// multiple output targets).
    ///
    /// Any target that the resolver cannot resolve is reported in `warnings`.
    pub(crate) fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        if let Some(deferred) = self.deferred.as_mut() {
            for xref in deferred.xrefs.iter_mut() {
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                });

                if xref.resolved.is_none() {
                    warnings.push(ReferenceWarning {
                        target: xref.target.clone(),
                        kind: ReferenceWarningKind::Unresolved,
                    });
                }
            }
        }

        self.rebuild_rendered(renderer);
    }

    /// Rebuilds [`Content::rendered`] from the deferred template and the
    /// current (resolved or unresolved) state of its cross-references.
    fn rebuild_rendered(&mut self, renderer: &dyn InlineSubstitutionRenderer) {
        let Some(deferred) = self.deferred.as_ref() else {
            return;
        };

        self.rendered = render_template(&deferred.template, &deferred.xrefs, renderer).into();
    }
}

/// Splices resolved (or fallback) cross-reference renderings into a placeholder
/// template, producing the final rendered text.
fn render_template(
    template: &str,
    xrefs: &[XrefSegment],
    renderer: &dyn InlineSubstitutionRenderer,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find(XREF_PLACEHOLDER_START) {
        out.push_str(&rest[..start]);
        let after = &rest[start + XREF_PLACEHOLDER_START.len_utf8()..];

        let Some(end) = after.find(XREF_PLACEHOLDER_END) else {
            // Malformed placeholder; emit the sentinel literally and continue.
            out.push(XREF_PLACEHOLDER_START);
            rest = after;
            continue;
        };

        let body = &after[..end];
        rest = &after[end + XREF_PLACEHOLDER_END.len_utf8()..];

        match body
            .parse::<usize>()
            .ok()
            .and_then(|index| xrefs.get(index))
        {
            Some(xref) => {
                renderer.render_xref(
                    &XrefRenderParams {
                        target: &xref.target,
                        provided_text: xref.provided_text.as_deref(),
                        resolved: xref.resolved.as_ref(),
                    },
                    &mut out,
                );
            }

            None => {
                // Unreachable while `template` and `xrefs` come from the same
                // `Content` (indices are assigned sequentially). If that
                // invariant is ever broken, emit the raw placeholder rather than
                // silently dropping the span, so the breakage is visible.
                debug_assert!(false, "xref placeholder index {body:?} out of range");
                out.push(XREF_PLACEHOLDER_START);
                out.push_str(body);
                out.push(XREF_PLACEHOLDER_END);
            }
        }
    }

    out.push_str(rest);
    out
}

impl<'src> From<Span<'src>> for Content<'src> {
    fn from(span: Span<'src>) -> Self {
        Self {
            original: span,
            rendered: CowStr::from(span.data()),
            deferred: None,
        }
    }
}

impl std::fmt::Debug for Content<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The deferred cross-reference state is an internal implementation
        // detail. It is omitted from the debug output unless present, so that
        // the (very common) cross-reference-free content debugs identically to
        // a plain `original` + `rendered` pair.
        let mut s = f.debug_struct("Content");
        s.field("original", &self.original);
        s.field("rendered", &self.rendered);

        if let Some(deferred) = self.deferred.as_ref() {
            s.field("deferred", deferred);
        }

        s.finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod is_empty {
        #[test]
        fn basic_empty_span() {
            let content = crate::content::Content::from(crate::Span::default());
            assert!(content.is_empty());
        }

        #[test]
        fn basic_non_empty_span() {
            let content = crate::content::Content::from(crate::Span::new("blah"));
            assert!(!content.is_empty());
        }
    }
}
