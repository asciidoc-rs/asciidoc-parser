//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use crate::{
    Span,
    content::InlineNode,
    parser::{
        InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarnings, ResolutionContext,
        XrefRenderParams,
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

    /// Source [`Span`] of each line that survived construction filtering, in
    /// the same order as the lines of [`rendered`](Self::rendered) at
    /// construction time.
    ///
    /// This is retained only so the attribute-references substitution can
    /// locate an `attribute-missing=warn` warning at the precise source
    /// offset of the offending `{name}` reference, rather than at the
    /// whole-content span. See
    /// [`apply_attributes`](crate::content::substitution_step) for the
    /// rationale and the correlation it performs.
    ///
    /// `None` when the content was not built line-by-line from document source
    /// (e.g. [`From<Span>`] or a table cell's pre-filtered value), in which
    /// case such warnings fall back to the whole-content span.
    source_lines: Option<Box<[Span<'src>]>>,

    /// Deferred cross-references discovered during substitution, awaiting
    /// resolution against a (possibly cross-document) catalog.
    ///
    /// `None` for the overwhelming majority of content, which contains no
    /// cross-references.
    deferred: Option<Box<DeferredContent>>,

    /// The semantic inline AST for this content, captured during substitution
    /// when the parser has [`capture_inline_ast`] enabled; `None` otherwise
    /// (the default). See [`inline_nodes`](Self::inline_nodes).
    ///
    /// [`capture_inline_ast`]: crate::Parser::with_inline_ast_capture
    inline_nodes: Option<Box<[InlineNode]>>,
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

    /// Target window selection, from a `window` attribute on the `xref:` macro
    /// (e.g. `_blank`). `None` for the shorthand form, which has no attribute
    /// list.
    pub(crate) window: Option<String>,

    /// Roles supplied via a `role` attribute on the `xref:` macro, if any.
    pub(crate) roles: Vec<String>,

    /// The cross-reference text style in effect for this reference: the
    /// `xrefstyle=` attribute on the `xref:` macro if given, otherwise the
    /// document-wide `xrefstyle` at the reference's location. `None` when
    /// `xrefstyle` is unset, in which case the target's reftext is used
    /// verbatim.
    pub(crate) xrefstyle: Option<crate::parser::XrefStyle>,

    /// The destination derived from the target itself, for a target that
    /// names a document. Computed during substitution, since it depends on the
    /// path attributes in effect at the reference; it is the fallback when
    /// [`resolved`](Self::resolved) is `None`.
    pub(crate) derived: Option<crate::parser::DerivedReference>,

    /// The resolved destination, filled in by resolution; `None` until then.
    pub(crate) resolved: Option<crate::parser::ResolvedReference>,
}

/// Sentinel codepoints (Unicode Private Use Area) bracketing a placeholder
/// index in [`DeferredContent::template`]. These cannot collide with user text
/// and are inert to the remaining substitution steps.
const XREF_PLACEHOLDER_START: char = '\u{E000}';
const XREF_PLACEHOLDER_END: char = '\u{E001}';

/// Sentinel codepoints (Unicode Private Use Area) bracketing a footnote's
/// rendered inline marker while a section title is being substituted. Like the
/// cross-reference placeholders above, these cannot collide with user text and
/// are inert to the remaining substitution steps.
///
/// A footnote in a section title is a real, document-order footnote, but its
/// marker must be kept out of the section's reference text and auto-generated
/// ID. Marking the marker in a single render (rather than re-rendering the
/// title with footnotes suppressed) means stateful substitutions — counters,
/// attribute references that expand into footnotes — run exactly once. See
/// [`strip_footnote_marker_spans`] and
/// [`Content::remove_footnote_marker_sentinels`].
pub(crate) const FOOTNOTE_MARKER_START: char = '\u{E002}';
pub(crate) const FOOTNOTE_MARKER_END: char = '\u{E003}';

/// Removes each footnote marker span — a [`FOOTNOTE_MARKER_START`] …
/// [`FOOTNOTE_MARKER_END`] region and everything between, i.e. the sentinels
/// *and* the marker they bracket — leaving footnote-free text suitable for a
/// section's reference text and auto-generated ID.
pub(crate) fn strip_footnote_marker_spans(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(start) = rest.find(FOOTNOTE_MARKER_START) {
        out.push_str(&rest[..start]);
        rest = &rest[start + FOOTNOTE_MARKER_START.len_utf8()..];

        // Drop through the matching end sentinel (the marker text). A start
        // without an end cannot occur — the substitution always emits both — but
        // if it somehow did, drop the remainder rather than reintroduce the
        // stray sentinel.
        rest = match rest.find(FOOTNOTE_MARKER_END) {
            Some(end) => &rest[end + FOOTNOTE_MARKER_END.len_utf8()..],
            None => "",
        };
    }

    out.push_str(rest);
    out
}

/// A fully-owned snapshot of a rendered title, including any deferred
/// cross-references it carries.
///
/// A block title stashed across a section heading (see
/// `Parser::pending_block_title`) cannot keep its borrowed [`Content`] — the
/// parser it rides on has no `'src` lifetime — so the title travels in this
/// owned form and is rebuilt into a [`Content`] (via
/// [`Content::from_owned_title`]) when the next block claims it. Carrying the
/// deferred template and cross-references along means an embedded `<<id>>`
/// still resolves once the catalog is complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedTitle {
    /// The rendered title text (the unresolved-fallback rendering when
    /// cross-references are present).
    rendered: String,

    /// The placeholder template and cross-references, when the title carries
    /// any; `None` for the (overwhelmingly common) cross-reference-free title.
    deferred: Option<(String, Vec<XrefSegment>)>,
}

impl<'src> Content<'src> {
    /// Constructs a `Content` from a source `Span` and a potentially-filtered
    /// view of that source text.
    pub(crate) fn from_filtered<T: AsRef<str>>(span: Span<'src>, filtered: T) -> Self {
        Self {
            original: span,
            rendered: filtered.as_ref().to_string().into(),
            source_lines: None,
            deferred: None,
            inline_nodes: None,
        }
    }

    /// Returns a fully-owned snapshot of this content's rendered text and
    /// deferred cross-references, for a title that must outlive its source
    /// borrow (see [`OwnedTitle`]).
    pub(crate) fn to_owned_title(&self) -> OwnedTitle {
        OwnedTitle {
            rendered: self.rendered.as_ref().to_string(),
            deferred: self
                .deferred
                .as_ref()
                .map(|d| (d.template.clone(), d.xrefs.clone())),
        }
    }

    /// Reconstitutes a [`Content`] from an [`OwnedTitle`] snapshot, anchored at
    /// `span`. The deferred cross-references (when present) are restored, so
    /// the document-order title pass can still resolve them.
    pub(crate) fn from_owned_title(span: Span<'src>, title: OwnedTitle) -> Self {
        Self {
            original: span,
            rendered: title.rendered.into(),
            source_lines: None,
            deferred: title
                .deferred
                .map(|(template, xrefs)| Box::new(DeferredContent { template, xrefs })),
            inline_nodes: None,
        }
    }

    /// Constructs a `Content` from a source `Span` and the per-line filtered
    /// view of that source, retaining the source `Span` of each surviving line.
    ///
    /// `line_spans` must contain one entry per line of `filtered_lines`, in the
    /// same order; each entry is the source span whose text is that filtered
    /// line (i.e. after any leading-indent stripping and trailing-whitespace
    /// trimming the caller applied). The retained spans let the
    /// attribute-references substitution report an `attribute-missing=warn`
    /// warning at the precise source offset of the offending reference; see
    /// [`apply_attributes`](crate::content::substitution_step).
    pub(crate) fn from_filtered_lines(
        span: Span<'src>,
        filtered_lines: &[&str],
        line_spans: Vec<Span<'src>>,
    ) -> Self {
        // One source span is required per filtered line; the default
        // `debug_assert_eq!` message reports both counts if this is ever broken.
        debug_assert_eq!(filtered_lines.len(), line_spans.len());

        Self {
            original: span,
            rendered: filtered_lines.join("\n").into(),
            source_lines: Some(line_spans.into_boxed_slice()),
            deferred: None,
            inline_nodes: None,
        }
    }

    /// Returns the original span from which this [`Content`] was derived.
    ///
    /// This is the source text before any substitions have been applied.
    pub fn original(&self) -> Span<'src> {
        self.original
    }

    /// Returns the source `Span` of each line that survived construction
    /// filtering, in rendered-line order, when they were retained (see
    /// [`from_filtered_lines`](Self::from_filtered_lines)).
    ///
    /// Used only by the attribute-references substitution to locate
    /// `attribute-missing=warn` warnings precisely.
    pub(crate) fn source_lines(&self) -> Option<&[Span<'src>]> {
        self.source_lines.as_deref()
    }

    /// Returns the final text after all substitutions have been applied.
    pub fn rendered(&'src self) -> &'src str {
        self.rendered.as_ref()
    }

    /// Returns the final rendered text, borrowed for the duration of `&self`
    /// rather than for `'src`.
    ///
    /// [`rendered`](Self::rendered) ties its result to `'src`, which a block's
    /// `title(&self)` accessor cannot provide. This shorter-lived borrow lets a
    /// block expose its title `Content`'s rendered text through the `&self`
    /// accessor.
    pub(crate) fn rendered_str(&self) -> &str {
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

    /// Returns the semantic inline AST for this content, or `None` when inline
    /// AST capture was not enabled on the parser.
    ///
    /// Enable capture with
    /// [`Parser::with_inline_ast_capture`](crate::Parser::with_inline_ast_capture).
    /// The tree exposes the inline structure (formatting, links,
    /// cross-references, footnotes, …) underneath [`rendered`](Self::rendered),
    /// with source targets and – for cross-references – resolved destinations
    /// once the document's references have been resolved.
    pub fn inline_nodes(&self) -> Option<&[InlineNode]> {
        self.inline_nodes.as_deref()
    }

    /// Installs the captured inline AST. Called once, from the substitution
    /// pipeline, when capture is enabled.
    pub(crate) fn set_inline_nodes(&mut self, nodes: Vec<InlineNode>) {
        self.inline_nodes = Some(nodes.into_boxed_slice());
    }

    /// Removes the [`FOOTNOTE_MARKER_START`]/[`FOOTNOTE_MARKER_END`] sentinels
    /// bracketing each footnote marker, *keeping* the marker itself, so the
    /// content renders normally. Called after a section title's reference text
    /// and ID have been derived (via [`strip_footnote_marker_spans`], which
    /// needs the sentinels to locate the markers). The sentinels are removed
    /// from the deferred template too, so a later cross-reference resolution
    /// rebuild does not reintroduce them.
    pub(crate) fn remove_footnote_marker_sentinels(&mut self) {
        if !self.rendered.as_ref().contains(FOOTNOTE_MARKER_START) {
            return;
        }

        self.rendered = self
            .rendered
            .as_ref()
            .replace([FOOTNOTE_MARKER_START, FOOTNOTE_MARKER_END], "")
            .into();

        if let Some(deferred) = self.deferred.as_mut() {
            deferred.template = deferred
                .template
                .replace([FOOTNOTE_MARKER_START, FOOTNOTE_MARKER_END], "");
        }
    }

    /// Returns the deferred cross-reference template and segments, if this
    /// content carries any.
    ///
    /// The template is the placeholder-bearing text captured by
    /// [`finalize_deferred`](Self::finalize_deferred); the segments are the
    /// cross-references in placeholder order. Used by the document-order title
    /// resolution pass, which re-renders a title's cross-references with
    /// cross-title (including circular) coordination that the per-content
    /// [`resolve_references`](Self::resolve_references) cannot provide.
    pub(crate) fn deferred_parts(&self) -> Option<(&str, &[XrefSegment])> {
        self.deferred
            .as_ref()
            .map(|d| (d.template.as_str(), d.xrefs.as_slice()))
    }

    /// Overwrites the rendered text directly.
    ///
    /// Used by the document-order title resolution pass, which computes a
    /// title's final rendering (coordinating cross-title references) and
    /// installs it here, in place of the per-content resolution that cannot see
    /// other titles.
    pub(crate) fn set_rendered(&mut self, rendered: String) {
        self.rendered = rendered.into();
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

    /// Applies `restore` to the explicit text of every deferred
    /// cross-reference.
    ///
    /// A deferred reference's text is captured out of the main rendered string
    /// during macro substitution, so passthrough placeholders inside it are not
    /// reached by the ordinary restore pass. This lets that pass reach them.
    pub(crate) fn restore_deferred_xref_passthroughs(
        &mut self,
        mut restore: impl FnMut(&mut String),
    ) {
        if let Some(deferred) = self.deferred.as_mut() {
            for xref in &mut deferred.xrefs {
                if let Some(text) = xref.provided_text.as_mut() {
                    restore(text);
                }
            }
        }
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
        warnings: &mut ReferenceWarnings<'src>,
    ) {
        let source = self.original;

        if let Some(deferred) = self.deferred.as_mut() {
            let DeferredContent { template, xrefs } = deferred.as_mut();

            // A `deferred` block always holds at least one xref placeholder, so
            // its finalized template is never empty. An empty template here
            // means `finalize_deferred` was skipped (a future-refactor hazard);
            // the `template.contains` guard below would then silently suppress
            // every unresolved-ref warning, so catch that invariant break in
            // debug builds.
            debug_assert!(!template.is_empty());

            for (index, xref) in xrefs.iter_mut().enumerate() {
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });

                // A reference whose placeholder is no longer in the template was
                // re-homed into a footnote (see `rehome_xref_placeholders`); the
                // footnote resolves and reports it, so it is not reported here.
                // A target that names a document is never reported: it
                // carries its own destination, so there was nothing here to
                // resolve.
                if xref.resolved.is_none()
                    && xref.derived.is_none()
                    && template.contains(&Content::xref_placeholder(index))
                {
                    warnings.unresolved(&xref.target, source);
                }
            }
        }

        self.rebuild_rendered(renderer);

        // Resolve the same cross-references in the captured inline AST, when
        // present, so its `Xref` nodes expose the resolved destination too.
        if let Some(nodes) = self.inline_nodes.as_deref_mut() {
            crate::content::resolve_xref_nodes(nodes, resolver);
        }
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

/// Re-homes the cross-reference placeholders found in `text` into a
/// self-contained (template, xrefs) pair.
///
/// When the cross-reference substitution runs before footnotes, a footnote's
/// text may carry placeholder tokens whose [`XrefSegment`]s live in the
/// enclosing block's cross-reference list (`all`). Because a footnote's text is
/// extracted out of the block, it needs its own copy of just those segments,
/// renumbered so its template is independent. This scans `text` for placeholder
/// tokens, clones the referenced segments into a fresh vector (in first-seen
/// order), and rewrites the tokens to the new local indices.
///
/// Text with no placeholders returns unchanged alongside an empty vector.
pub(crate) fn rehome_xref_placeholders(
    text: &str,
    all: &[XrefSegment],
) -> (String, Vec<XrefSegment>) {
    let mut local: Vec<XrefSegment> = vec![];

    if !text.contains(XREF_PLACEHOLDER_START) {
        return (text.to_string(), local);
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find(XREF_PLACEHOLDER_START) {
        out.push_str(&rest[..start]);
        let after = &rest[start + XREF_PLACEHOLDER_START.len_utf8()..];

        let Some(end) = after.find(XREF_PLACEHOLDER_END) else {
            out.push(XREF_PLACEHOLDER_START);
            rest = after;
            continue;
        };

        let body = &after[..end];
        rest = &after[end + XREF_PLACEHOLDER_END.len_utf8()..];

        match body.parse::<usize>().ok().and_then(|index| all.get(index)) {
            Some(segment) => {
                let local_index = local.len();
                local.push(segment.clone());
                out.push_str(&Content::xref_placeholder(local_index));
            }

            None => {
                out.push(XREF_PLACEHOLDER_START);
                out.push_str(body);
                out.push(XREF_PLACEHOLDER_END);
            }
        }
    }

    out.push_str(rest);
    (out, local)
}

/// Splices resolved (or fallback) cross-reference renderings into a placeholder
/// template, producing the final rendered text.
///
/// This is the seam used by the document-order title resolution pass: it hands
/// in a title's captured template together with a set of [`XrefSegment`]s whose
/// [`resolved`](XrefSegment::resolved) fields it has filled in with cross-title
/// (including circular) coordination, and receives the final rendered title.
pub(crate) fn render_xref_template(
    template: &str,
    xrefs: &[XrefSegment],
    renderer: &dyn InlineSubstitutionRenderer,
) -> String {
    render_template(template, xrefs, renderer)
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
                        window: xref.window.as_deref(),
                        roles: &xref.roles,
                        xrefstyle: xref.xrefstyle,
                        derived: xref.derived.as_ref(),
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

/// The deferred cross-references carried by a footnote's text.
///
/// A footnote's text is extracted out of the flow of the block during the
/// macros substitution step, so any cross-reference (`<<id>>`, `xref:id[…]`)
/// inside it cannot be resolved by the document-level pass that resolves
/// references in block content. Instead, the footnote captures its
/// cross-references here — as a placeholder template plus the references in
/// placeholder order — and they are resolved alongside the block references
/// (see [`Footnote::resolve_references`]).
///
/// [`Footnote::resolve_references`]: crate::document::Footnote::resolve_references
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FootnoteDeferred {
    /// The footnote text with opaque placeholder tokens marking where each
    /// cross-reference will be spliced in.
    template: String,

    /// The cross-references, in placeholder order.
    xrefs: Vec<XrefSegment>,
}

impl FootnoteDeferred {
    /// Constructs a footnote's deferred cross-reference state from the
    /// placeholder-bearing `template` and its `xrefs` (in placeholder order).
    pub(crate) fn new(template: String, xrefs: Vec<XrefSegment>) -> Self {
        Self { template, xrefs }
    }

    /// Renders the footnote text from the template and the current (resolved or
    /// unresolved) state of its cross-references.
    pub(crate) fn render(&self, renderer: &dyn InlineSubstitutionRenderer) -> String {
        render_template(&self.template, &self.xrefs, renderer)
    }

    /// Resolves the footnote's cross-references using `resolver`, reporting any
    /// unresolved target in `warnings` against `source`. Rendering the resolved
    /// text is left to the caller (via [`render`](Self::render)).
    pub(crate) fn resolve<'src>(
        &mut self,
        resolver: &dyn ReferenceResolver,
        warnings: &mut ReferenceWarnings<'src>,
        source: Span<'src>,
    ) {
        for xref in self.xrefs.iter_mut() {
            xref.resolved = resolver.resolve(&ResolutionContext {
                target: &xref.target,
                provided_text: xref.provided_text.as_deref(),
                derived: xref.derived.as_ref(),
            });

            if xref.resolved.is_none() && xref.derived.is_none() {
                warnings.unresolved(&xref.target, source);
            }
        }
    }
}

impl std::fmt::Debug for FootnoteDeferred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FootnoteDeferred")
            .field("template", &self.template)
            .field("xrefs", &self.xrefs)
            .finish()
    }
}

impl<'src> From<Span<'src>> for Content<'src> {
    fn from(span: Span<'src>) -> Self {
        Self {
            original: span,
            rendered: CowStr::from(span.data()),
            source_lines: None,
            deferred: None,
            inline_nodes: None,
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

    mod strip_footnote_marker_spans {
        use super::super::{
            FOOTNOTE_MARKER_END, FOOTNOTE_MARKER_START, strip_footnote_marker_spans,
        };

        fn marked(marker: &str) -> String {
            format!("{FOOTNOTE_MARKER_START}{marker}{FOOTNOTE_MARKER_END}")
        }

        #[test]
        fn leaves_unmarked_text_unchanged() {
            assert_eq!(strip_footnote_marker_spans("Plain title"), "Plain title");
        }

        #[test]
        fn removes_a_marker_span_and_its_sentinels() {
            let input = format!("Title{}", marked("[1]"));
            assert_eq!(strip_footnote_marker_spans(&input), "Title");
        }

        #[test]
        fn removes_multiple_spans_keeping_surrounding_text() {
            let input = format!("a{}b{}c", marked("[1]"), marked("[2]"));
            assert_eq!(strip_footnote_marker_spans(&input), "abc");
        }

        #[test]
        fn a_start_without_an_end_drops_the_remainder() {
            // Defensive: the substitution always emits balanced sentinels, but a
            // lone start must not leak the sentinel into the output.
            let input = format!("Title{FOOTNOTE_MARKER_START}dangling");
            assert_eq!(strip_footnote_marker_spans(&input), "Title");
        }
    }

    mod footnote_deferred {
        use super::super::{
            FootnoteDeferred, XREF_PLACEHOLDER_END, XREF_PLACEHOLDER_START, XrefSegment,
            rehome_xref_placeholders,
        };

        fn segment(target: &str) -> XrefSegment {
            XrefSegment {
                target: target.to_string(),
                provided_text: None,
                window: None,
                roles: vec![],
                xrefstyle: None,
                derived: None,
                resolved: None,
            }
        }

        #[test]
        fn rehomes_a_placeholder_into_a_local_segment() {
            let all = vec![segment("a"), segment("b")];
            // Reference only the second segment; it becomes local index 0.
            let text = format!("see {XREF_PLACEHOLDER_START}1{XREF_PLACEHOLDER_END} here");

            let (template, local) = rehome_xref_placeholders(&text, &all);

            assert_eq!(local.len(), 1);
            assert_eq!(local.first().unwrap().target, "b");
            assert_eq!(
                template,
                format!("see {XREF_PLACEHOLDER_START}0{XREF_PLACEHOLDER_END} here")
            );
        }

        #[test]
        fn text_without_placeholders_is_returned_unchanged() {
            let (template, local) = rehome_xref_placeholders("plain text", &[segment("a")]);
            assert_eq!(template, "plain text");
            assert!(local.is_empty());
        }

        #[test]
        fn malformed_placeholders_are_passed_through_literally() {
            // A non-numeric index and an unterminated placeholder are both left
            // as-is (these cannot arise in practice, but the fallback is exercised).
            let bad_index = format!("a{XREF_PLACEHOLDER_START}xyz{XREF_PLACEHOLDER_END}b");
            let (template, local) = rehome_xref_placeholders(&bad_index, &[]);
            assert_eq!(template, bad_index);
            assert!(local.is_empty());

            let unterminated = format!("a{XREF_PLACEHOLDER_START}0 no end");
            let (template, local) = rehome_xref_placeholders(&unterminated, &[]);
            assert_eq!(template, unterminated);
            assert!(local.is_empty());
        }

        #[test]
        fn out_of_range_placeholder_index_is_passed_through() {
            // An index with no matching segment in `all` is left literal.
            let text = format!("x{XREF_PLACEHOLDER_START}9{XREF_PLACEHOLDER_END}y");
            let (template, local) = rehome_xref_placeholders(&text, &[segment("a")]);
            assert_eq!(template, text);
            assert!(local.is_empty());
        }

        #[test]
        fn debug_includes_template_and_xrefs() {
            let deferred = FootnoteDeferred::new("t".to_string(), vec![segment("a")]);
            let rendered = format!("{deferred:?}");
            assert!(rendered.contains("FootnoteDeferred"));
            assert!(rendered.contains("template"));
        }
    }
}
