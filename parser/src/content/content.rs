//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use crate::{
    Parser, Span,
    content::Passthrough,
    inlines::{InlineNode, RefVariant},
    parser::{
        InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarnings, ResolutionContext,
        ResolvedAttributes, ResolvedReference, XrefRenderParams,
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
/// [`Document::resolve_references`] — at which point [`rendered_html()`]
/// reflects the resolved links. Until then, [`rendered_html()`] shows an
/// unresolved fallback, so it always returns clean text.
///
/// [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/
/// [`SimpleBlock`]: crate::blocks::SimpleBlock
/// [`RawDelimitedBlock`]: crate::blocks::RawDelimitedBlock
/// [`Document::resolve_references`]: crate::Document::resolve_references
/// [`rendered_html()`]: Self::rendered_html
#[derive(Clone)]
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

    /// Inline passthroughs (`+++…+++`, `++…++`, `$$…$$`, `pass:[…]`, inline
    /// STEM macros) extracted from this content during substitution, in the
    /// order they were extracted.
    ///
    /// The substitution pipeline pulls each passthrough out before running the
    /// other substitutions and splices it back in afterward; this retains the
    /// collection so it is observable after the fact (see
    /// [`passthroughs`](Self::passthroughs)), analogous to Asciidoctor's
    /// internal `@passthroughs` array. Empty for the common case of content
    /// with no passthroughs, and for content whose substitution group does not
    /// extract them.
    passthroughs: Vec<Passthrough>,

    /// The inline AST for this content: the structured representation of its
    /// inline nodes, built by the single-pass builder
    /// (the crate-internal `inline_builder` module) directly from the
    /// pre-substitution source, in parallel with the substitution pass that
    /// produced [`rendered`](Self::rendered).
    ///
    /// This is a **derived artifact** — built alongside the rendered content,
    /// which remains the source of truth (making the tree canonical, with
    /// `rendered_html()` a fold of it, is the remaining half of the [inline
    /// AST architecture] design's step 6). Every parse builds it: the
    /// `with_inline_tree` opt-in that used to gate it is retired. Because it is
    /// derived, it is deliberately excluded from [`PartialEq`]/[`Eq`]/[`Hash`]:
    /// two `Content`s with equal rendered text compare equal regardless of
    /// their trees.
    ///
    /// [inline AST architecture]: https://github.com/scouten/asciidoc-parser/blob/main/docs/design/inline-ast-architecture.md
    inlines: Vec<InlineNode<'src>>,

    /// The document attributes as of the point in the document this content
    /// was parsed — the *order-dependent* half of what a fold of
    /// [`inlines`](Self::inlines) needs, and the reason it can be folded
    /// **later than its parse**.
    ///
    /// Retained only for content carrying a deferred cross-reference, which is
    /// the only content whose rendering is rebuilt after the parse (see
    /// [`resolve_references`](Self::resolve_references)); `None` for everything
    /// else, which is nearly everything.
    ///
    /// It must be taken *here* rather than read back at resolution time,
    /// because document attributes are mutable parse state: a `:imagesdir:` or
    /// `:icons:` line rebinds them for everything after it, so what is in
    /// effect at the end of a document is not generally what was in effect
    /// where this content was written.
    ///
    /// # Why the attributes and not a whole [`RenderContext`](crate::parser::RenderContext)
    ///
    /// A fold also needs the path resolver and the file handlers. Those are
    /// *parse-wide configuration* rather than document state — they cannot
    /// change mid-parse — so nothing is lost by not freezing them, and freezing
    /// them here would cost something real: they are `Rc<dyn …>`, so retaining
    /// them would make [`Content`], and with it [`Document`](crate::Document),
    /// no longer [`Send`]/[`Sync`]. A `Document` is both today
    /// (`document_stays_send_and_sync` pins it), which is worth more than
    /// saving the caller from supplying the parser it already holds. The
    /// increment that folds at resolution time assembles a `RenderContext`
    /// from this plus that configuration.
    ///
    /// Boxed for the same reason [`deferred`](Self::deferred) is, and shared
    /// from the parser by [`Arc`](std::sync::Arc) internally, so retaining one
    /// allocates nothing beyond the box.
    ///
    /// Like [`inlines`](Self::inlines), it is derived rather than identifying,
    /// so it is excluded from [`PartialEq`]/[`Eq`]/[`Hash`] and from [`Debug`].
    render_attributes: Option<Box<ResolvedAttributes>>,
}

/// The deferred (cross-reference-bearing) portion of a [`Content`].
///
/// The cross-references are read off the content's own **inline tree** (see
/// [`block_tree_xref_segments`] and [`footnote_tree_xref_segments`]), which is
/// what design §5.2's survey named as the first of the six things
/// `run_pipeline` still solely owned. They arrive already partitioned into the
/// two lists resolution keeps apart, where the string pipeline produced one
/// flat list that had to be split by asking which placeholders its template
/// still spliced.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DeferredContent {
    /// The **block-level** cross-references, in the order
    /// [`block_tree_xref_segments`] visits them — which is the order
    /// [`Content::mirror_tree_xref_resolution`] installs destinations back in,
    /// the two being the same walk.
    block: Vec<XrefSegment>,

    /// The cross-references this content's **footnotes** carry. A footnote's
    /// text is extracted out of the flow, so these are absent from
    /// [`block`](Self::block) and are installed into the tree's footnote
    /// subtrees instead.
    footnote: Vec<XrefSegment>,

    /// The string pipeline's own placeholder template, still captured by
    /// [`Content::finalize_deferred`] as it always was.
    ///
    /// Nothing on the production path renders from it any more except the one
    /// content that has no tree to fold: a block title carried across a section
    /// heading, which travels through `Parser::pending_block_title` as an
    /// [`OwnedTitle`] because the parser it rides on has no `'src` lifetime,
    /// and so arrives at the claiming block with its inline nodes dropped. Its
    /// rendering cannot be a fold, so it stays a splice — see
    /// [`Content::rebuild_rendered`]. It goes when that title keeps its tree,
    /// which is what deletes design §4.2's second sentinel system outright.
    ///
    /// Empty between [`Content::set_deferred_xrefs`] recording the list and
    /// `finalize_deferred` capturing the template, which is within one
    /// `run_pipeline` call and reaches no reader.
    template: String,

    /// Whether [`block`](Self::block) and [`footnote`](Self::footnote) were
    /// read off this content's **inline tree**, rather than being the string
    /// pipeline's own flat list.
    ///
    /// `false` is the carve-out design §4.2's second sentinel system still
    /// keeps: the single-pass builder leaves a documented set of forms
    /// unrecognized, so the tree can hold *fewer* cross-references than the
    /// string pipeline deferred. Where it does, the tree is known not to
    /// describe this content — installing its list would silently drop a
    /// cross-reference and folding it would render the construct as literal
    /// source — so the string pipeline's answer stands, on the placeholder
    /// path, exactly as before this increment
    /// (`xref_mirror_is_skipped_when_the_tree_defers_a_reference_form`).
    ///
    /// The carve-out narrows on its own as each builder prep lands, and
    /// disappears when none is left — which is what finally deletes this
    /// sentinel system.
    from_tree: bool,

    /// The string pipeline's own flat list, in placeholder order — the answer
    /// [`Content::set_tree_xrefs`] overwrote.
    ///
    /// It is kept for one reason: the differential corpus that compares the two
    /// derivations
    /// ([`inline_builder_xref_segment_parity`](crate::tests)) needs a golden,
    /// and once the tree's answer *is* the production answer it would otherwise
    /// be comparing the tree against itself and passing for that reason — the
    /// exact failure the frozen recordings exist to prevent. This is the same
    /// move `Passthroughs::observable` made when
    /// [`Content::passthroughs`](Content::passthroughs) became a view over the
    /// tree, for the same reason, and it goes the same way: with
    /// `run_pipeline` itself.
    ///
    /// It takes part in the derived [`PartialEq`]/[`Eq`]/[`Hash`]/[`Debug`]
    /// like any other field, so two `DeferredContent`s compare on it in a test
    /// build and cannot in a release one. That is harmless here because both
    /// sides of every comparison in this crate come from the same pipeline run,
    /// and writing the four impls out by hand to exclude one transitional field
    /// would cost more than it buys.
    #[cfg(test)]
    string_xrefs: Vec<XrefSegment>,
}

/// A read-only view of a [`Content`]'s deferred cross-references — the shape
/// [`Content::deferred_parts`] hands out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DeferredParts<'a> {
    /// The block-level cross-references, in document order.
    pub(crate) block: &'a [XrefSegment],

    /// The cross-references this content's footnotes carry.
    pub(crate) footnote: &'a [XrefSegment],

    /// The placeholder template, for a content that renders from one — see
    /// [`DeferredContent::template`].
    pub(crate) template: &'a str,

    /// Whether [`block`](Self::block) and [`footnote`](Self::footnote) were
    /// read off the content's inline tree — see
    /// [`DeferredContent::from_tree`].
    pub(crate) from_tree: bool,

    /// The string pipeline's own flat list — see
    /// [`DeferredContent::string_xrefs`].
    #[cfg(test)]
    pub(crate) string_xrefs: &'a [XrefSegment],
}

/// A single deferred cross-reference.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

/// Strips markup down to plain text, mirroring Asciidoctor's
/// `Document::Title` sanitize option (`XmlSanitizeRx = /<[^>]+>/`): every tag
/// — opening, closing, or self-contained (e.g. an `<img>` rendered from an
/// inline `image:` macro) — is removed, any run of interior spaces left
/// behind by a removed tag is squeezed to one, and the result is trimmed.
///
/// A value with no `<` is returned unchanged, matching Asciidoctor, which
/// skips the squeeze-and-trim pass entirely when there is nothing to
/// sanitize. Used both by the sanitized doctitle accessor
/// (`Document::doctitle_sanitized`) and by this document's own
/// cross-reference fallback text (`this_document_reference`), which embeds
/// the doctitle inside its own `<a>` and so cannot carry nested markup.
pub(crate) fn sanitize_title(source: &str) -> String {
    if !source.contains('<') {
        return source.to_string();
    }

    let mut stripped = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(lt) = rest.find('<') {
        stripped.push_str(&rest[..lt]);

        let after_lt = &rest[lt + 1..];
        match after_lt.find('>') {
            // `[^>]+` requires at least one character between `<` and `>`;
            // an empty `<>` does not match and is copied through verbatim.
            Some(gt) if gt > 0 => rest = &after_lt[gt + 1..],
            _ => {
                stripped.push('<');
                rest = after_lt;
            }
        }
    }
    stripped.push_str(rest);

    let mut squeezed = String::with_capacity(stripped.len());
    let mut prev_was_space = false;
    for c in stripped.chars() {
        if c == ' ' {
            if prev_was_space {
                continue;
            }
            prev_was_space = true;
        } else {
            prev_was_space = false;
        }
        squeezed.push(c);
    }

    squeezed.trim().to_string()
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

    /// The deferred cross-references, when the title carries any; `None` for
    /// the (overwhelmingly common) cross-reference-free title.
    ///
    /// The inline **tree** they were read off does not travel with them — its
    /// nodes borrow `'src` — so a carried title is the one content whose
    /// rendering cannot be a fold, and the
    /// [`template`](DeferredContent::template) is what it renders from
    /// instead.
    deferred: Option<DeferredContent>,

    /// The document attributes the title's own content carried, so the rebuilt
    /// [`Content`] can still be folded later than its parse — see
    /// [`Content::render_attributes`]. Travels with `deferred`, since the two
    /// are populated together and a title carrying no deferred
    /// cross-reference is never re-rendered.
    render_attributes: Option<Box<ResolvedAttributes>>,
}

impl<'src> Content<'src> {
    /// Constructs a `Content` from a source `Span` and a potentially-filtered
    /// view of that source text.
    pub(crate) fn from_filtered<T: AsRef<str>>(span: Span<'src>, filtered: T) -> Self {
        let filtered = filtered.as_ref();

        // When filtering was a no-op — the filtered text is byte-identical to
        // the source span — borrow the span rather than allocating an owned
        // copy of text we already hold.
        let rendered = if filtered == span.data() {
            CowStr::Borrowed(span.data())
        } else {
            filtered.to_string().into()
        };

        Self {
            original: span,
            rendered,
            source_lines: None,
            deferred: None,
            passthroughs: Vec::new(),
            inlines: Vec::new(),
            render_attributes: None,
        }
    }

    /// The document attributes as of this content's own parse, when they were
    /// retained — see [`render_attributes`](Self::render_attributes) (the
    /// field).
    ///
    /// `Some` exactly for content carrying a deferred cross-reference.
    // Consumed only by tests until the deferred-cross-reference sentinel
    // system's retirement (design §4.2's second) folds at resolution time —
    // the same staging the `inline_builder` module's fold and side effects
    // are under.
    #[allow(dead_code)]
    pub(crate) fn render_attributes(&self) -> Option<&ResolvedAttributes> {
        self.render_attributes.as_deref()
    }

    /// Retains `attributes` as this content's document-attribute state, for
    /// the fold that will run after resolution.
    ///
    /// Called once, from
    /// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup),
    /// after the pipeline has run and the deferred cross-references (if any)
    /// are known.
    pub(crate) fn set_render_attributes(&mut self, attributes: ResolvedAttributes) {
        self.render_attributes = Some(Box::new(attributes));
    }

    /// Returns a fully-owned snapshot of this content's rendered text and
    /// deferred cross-references, for a title that must outlive its source
    /// borrow (see [`OwnedTitle`]).
    pub(crate) fn to_owned_title(&self) -> OwnedTitle {
        OwnedTitle {
            rendered: self.rendered.as_ref().to_string(),
            deferred: self.deferred.as_ref().map(|d| (**d).clone()),
            render_attributes: self.render_attributes.clone(),
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
            deferred: title.deferred.map(Box::new),
            passthroughs: Vec::new(),
            inlines: Vec::new(),
            render_attributes: title.render_attributes,
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
        filtered_lines: &[&'src str],
        line_spans: Vec<Span<'src>>,
    ) -> Self {
        // One source span is required per filtered line; the default
        // `debug_assert_eq!` message reports both counts if this is ever broken.
        debug_assert_eq!(filtered_lines.len(), line_spans.len());

        // A single surviving line needs no join: it is already a contiguous
        // `'src` slice, so borrow it rather than allocating an owned copy. This
        // is the common plain-prose paragraph case; only a genuinely multi-line
        // block pays for the `join`.
        let rendered = match filtered_lines {
            [only] => CowStr::Borrowed(only),
            _ => filtered_lines.join("\n").into(),
        };

        Self {
            original: span,
            rendered,
            source_lines: Some(line_spans.into_boxed_slice()),
            deferred: None,
            passthroughs: Vec::new(),
            inlines: Vec::new(),
            render_attributes: None,
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

    /// Returns the default **HTML** rendering of this content: the final text
    /// after all substitutions have been applied.
    ///
    /// This is the built-in HTML output. (A custom
    /// [`InlineSubstitutionRenderer`] installed via
    /// [`Parser::with_inline_substitution_renderer`](crate::Parser::with_inline_substitution_renderer)
    /// still drives this output during migration; moving renderer selection to
    /// render time is a later step of the [inline AST architecture].)
    ///
    /// [inline AST architecture]: https://github.com/scouten/asciidoc-parser/blob/main/docs/design/inline-ast-architecture.md
    pub fn rendered_html(&'src self) -> &'src str {
        self.rendered.as_ref()
    }

    /// Returns the final rendered text, borrowed for the duration of `&self`
    /// rather than for `'src`.
    ///
    /// [`rendered_html`](Self::rendered_html) ties its result to `'src`, which
    /// a block's `title(&self)` accessor cannot provide. This shorter-lived
    /// borrow lets a block expose its title `Content`'s rendered text
    /// through the `&self` accessor.
    pub(crate) fn rendered_str(&self) -> &str {
        self.rendered.as_ref()
    }

    /// Returns an owned copy of the final text after all substitutions have
    /// been applied.
    ///
    /// Unlike [`rendered_html()`](Self::rendered_html), this does not tie the
    /// returned value to the `'src` lifetime, so it can be called on a
    /// short-lived `Content` built solely to render a fragment (e.g. a
    /// block's attribution or citation text).
    pub(crate) fn rendered_owned(&self) -> String {
        self.rendered.as_ref().to_string()
    }

    /// Returns `true` if `self` contains no text.
    pub fn is_empty(&self) -> bool {
        self.rendered.as_ref().is_empty()
    }

    /// Returns the inline passthroughs this content holds, in **document
    /// order**.
    ///
    /// An inline passthrough (`+++…+++`, `++…++`, `$$…$$`, `pass:[…]`, or an
    /// inline STEM macro) is pulled out of the text before the other
    /// substitutions run and spliced back in afterward. This exposes that
    /// collection — each entry's stored [`text`](Passthrough::text) and
    /// resolved [`subs`](Passthrough::subs) — for inspection, analogous to
    /// Asciidoctor's internal `@passthroughs` array.
    ///
    /// The slice is empty when the content has no passthroughs, and when the
    /// content's substitution group does not extract passthroughs at all (only
    /// groups that include the
    /// [macros](crate::content::SubstitutionStep::Macros) step, or the
    /// header group, do).
    ///
    /// # Order
    ///
    /// The entries are in the order the author wrote them. This is a view over
    /// the [inline tree](Self::inlines) rather than the substitution
    /// pipeline's own extraction list, and the two orders differ: the bare
    /// `+…+` form is extracted in a second pass and STEM in a third, so
    /// `+++A+++ and stem:[B] and [x-]++C++ and ++D++` extracts as `A, C, D, B`
    /// where this returns `A, B, C, D`. Extraction order was an artifact of
    /// that multi-pass implementation, never a documented property of this
    /// method.
    pub fn passthroughs(&self) -> &[Passthrough] {
        &self.passthroughs
    }

    /// Records the inline passthroughs read off this content's tree, so they
    /// remain observable via [`passthroughs`](Self::passthroughs) after the
    /// parse.
    pub(crate) fn set_passthroughs(&mut self, passthroughs: Vec<Passthrough>) {
        self.passthroughs = passthroughs;
    }

    /// Returns the inline AST for this content: the structured, read-only
    /// representation of its inline nodes.
    ///
    /// Every parse builds this — the `with_inline_tree` opt-in that used to
    /// gate it is retired — so it is an empty slice only for content whose tree
    /// is genuinely empty (empty content). The tree is built by the single-pass
    /// builder
    /// (the crate-internal `inline_builder` module) directly from the
    /// pre-substitution source, so each node carries its own precise source
    /// [`Span`] (a node born from a transformation, such as an attribute
    /// expansion, falls back to a documented coarser span) and a macro node
    /// carries its own parsed attribute list. The fold of the tree reproduces
    /// [`rendered_html`](Self::rendered_html) byte-for-byte across the
    /// builder's supported vocabulary; a small set of forms is documented as
    /// deferred (documented in the `inline_builder`
    /// module), each left as literal text in the tree rather than a wrong
    /// node. The tree is not yet the canonical representation (see the
    /// [inline AST architecture design], Phase 4 step 6).
    ///
    /// Cross-references in the tree carry their resolved destination once a
    /// full [`Parser::parse`](crate::Parser::parse) has resolved the document's
    /// references: each resolved destination is mirrored into the corresponding
    /// [`Ref`](crate::inlines::Ref) node, so a caller that walks
    /// [`inlines`](Self::inlines) after the parse sees the same destinations
    /// the rendered string reflects (§4.3 of the design). Before resolution
    /// — or for a standalone parse with no document catalog — a `Ref`
    /// node's destination is `None`.
    ///
    /// [inline AST architecture design]: https://github.com/scouten/asciidoc-parser/blob/main/docs/design/inline-ast-architecture.md
    pub fn inlines(&self) -> &[InlineNode<'src>] {
        &self.inlines
    }

    /// Installs the inline AST built for this content by the single-pass
    /// builder (see [`inline_builder`](crate::content::inline_builder)).
    pub(crate) fn set_inlines(&mut self, inlines: Vec<InlineNode<'src>>) {
        self.inlines = inlines;
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
    pub(crate) fn deferred_parts(&self) -> Option<DeferredParts<'_>> {
        self.deferred.as_ref().map(|d| DeferredParts {
            block: &d.block,
            footnote: &d.footnote,
            template: &d.template,
            from_tree: d.from_tree,
            #[cfg(test)]
            string_xrefs: &d.string_xrefs,
        })
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
        self.deferred.as_ref().is_some_and(|d| {
            d.block
                .iter()
                .chain(d.footnote.iter())
                .any(|x| x.resolved.is_none())
        })
    }

    /// Records the cross-references the **string pipeline's** macros step
    /// discovered, in placeholder order. The placeholder tokens must already
    /// have been written into [`Content::rendered`], in the same order.
    ///
    /// This is no longer what a caller reads: the production list is read off
    /// the inline tree a moment later (see
    /// [`set_tree_xrefs`](Self::set_tree_xrefs), which overwrites this).
    /// What it still does is give the string pipeline its own answer — the
    /// golden every differential corpus on this branch takes through
    /// [`apply_string_pipeline`](crate::content::SubstitutionGroup) — and carry
    /// the placeholder template that a content with no tree renders from.
    ///
    /// The list lands in [`block`](DeferredContent::block) because the string
    /// pipeline does not partition: a footnote-embedded reference is told apart
    /// by its placeholder having left the template, which is what
    /// [`template_splices`](Self::template_splices) reads it back out with.
    pub(crate) fn set_deferred_xrefs(&mut self, xrefs: Vec<XrefSegment>) {
        if xrefs.is_empty() {
            return;
        }

        debug_assert!(
            self.deferred.is_none(),
            "set_deferred_xrefs must be called at most once per Content"
        );

        self.deferred = Some(Box::new(DeferredContent {
            #[cfg(test)]
            string_xrefs: Vec::new(),
            block: xrefs,
            footnote: Vec::new(),
            template: String::new(),
            from_tree: false,
        }));
    }

    /// Installs the deferred cross-references **read off this content's inline
    /// tree**, replacing whatever the string pipeline's own macros step left
    /// here.
    ///
    /// This is design §5.2's survey item, wired: the two lists come from
    /// [`block_tree_xref_segments`] and [`footnote_tree_xref_segments`], which
    /// have been staged and unwired since they landed — the same staging every
    /// recognition side effect was under before it was re-attached.
    ///
    /// The string pipeline still produces its own list (it is the differential
    /// corpora's golden until `run_pipeline` is deleted), and this overwrites
    /// it. What is *kept* from it is the placeholder template, and only where
    /// the template's own placeholders line up one-to-one with `block` — see
    /// [`DeferredContent::template`] for the one content that reads it and why
    /// a mismatch must drop it rather than splice into it.
    ///
    /// A content whose tree holds no cross-reference at all clears the deferred
    /// state outright: a construct the builder does not recognize is one this
    /// content no longer defers, and it renders as the fold leaves it.
    pub(crate) fn set_tree_xrefs(
        &mut self,
        tree: &[InlineNode<'src>],
        renderer: &dyn InlineSubstitutionRenderer,
        context: &crate::parser::RenderContext,
    ) {
        // The string pipeline deferring nothing here means this content defers
        // nothing at all: the builder recognizes a *subset* of the forms
        // `InlineXrefReplacer` does — that containment is the premise the
        // carve-out below rests on — so a tree holding a cross-reference node
        // the replacer did not defer cannot arise.
        //
        // This is why the two walks happen **after** this early return rather
        // than at the call site: they traverse the whole tree, and every
        // paragraph in every document would otherwise pay for two traversals
        // that this invariant says could only ever come back empty. The saving
        // is structural rather than measured — the repository's own benchmarks
        // cannot resolve it, since a base-against-itself control run on the
        // machine used here reported a significant 3% "improvement" on
        // byte-identical code.
        let Some(previous) = self.deferred.take() else {
            return;
        };

        let block = block_tree_xref_segments(tree, renderer, context);
        let footnote = footnote_tree_xref_segments(tree, renderer, context);

        // The carve-out: where the tree holds fewer cross-references than the
        // string pipeline deferred, it is known not to describe this content,
        // so its list is *not* installed and the string pipeline's answer
        // stands. See `DeferredContent::from_tree`.
        if Self::template_splices(&previous) != block.len() {
            self.deferred = Some(previous);
            return;
        }

        self.deferred = Some(Box::new(DeferredContent {
            #[cfg(test)]
            string_xrefs: previous.string_xrefs,
            block,
            footnote,
            template: previous.template,
            from_tree: true,
        }));
    }

    /// How many placeholders the string pipeline's template still splices.
    ///
    /// Its flat list is indexed by *placeholder*, so "the block-level ones" are
    /// those whose placeholder survived the footnote re-homing pass — the count
    /// the tree's own block-level list has to match for the two to describe the
    /// same content. Called only while `previous` is still the string
    /// pipeline's own answer, where `block` is that flat list.
    fn template_splices(previous: &DeferredContent) -> usize {
        (0..previous.block.len())
            .filter(|index| {
                previous
                    .template
                    .contains(&Content::xref_placeholder(*index))
            })
            .count()
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
        let template = self.rendered.as_ref().to_string();

        {
            let Some(deferred) = self.deferred.as_mut() else {
                return;
            };

            deferred.template = template;

            // Snapshot the string pipeline's own answer here rather than where
            // the list was recorded: this runs at the very end of
            // `run_pipeline`, so the passthrough restore pass
            // (`restore_deferred_xref_passthroughs`) has already reached into
            // each segment's text. A snapshot taken earlier would hand the
            // differential corpus a golden still carrying `\u{96}`…`\u{97}`
            // sentinels.
            #[cfg(test)]
            {
                deferred.string_xrefs = deferred.block.clone();
            }
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
            for xref in &mut deferred.block {
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
        parser: &Parser,
    ) {
        let source = self.original;

        if let Some(deferred) = self.deferred.as_mut() {
            // Where the two lists were read off the tree they arrive already
            // partitioned, and only the block-level ones are reported here: a
            // footnote's own copy of an embedded reference resolves and reports
            // it. Where they are still the string pipeline's flat list, that
            // same split is read out of the template — a placeholder that has
            // left it was re-homed onto a footnote — which is what
            // `reports_unresolved` answers for either shape.
            let from_tree = deferred.from_tree;
            let template = deferred.template.clone();

            for (index, xref) in deferred.block.iter_mut().enumerate() {
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });

                let reports_unresolved =
                    from_tree || template.contains(&Content::xref_placeholder(index));

                // A target that names a document is never reported: it carries
                // its own destination, so there was nothing here to resolve.
                if xref.resolved.is_none() && xref.derived.is_none() && reports_unresolved {
                    warnings.unresolved(&xref.target, source);
                }
            }

            for xref in deferred.footnote.iter_mut() {
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });
            }
        }

        let from_tree = self.resolve_tree_references();

        // Exactly **one** of the two renderings runs, and which one is now a
        // question about the *tree* rather than about the cross-references in
        // it: a content with a tree folds it, and a content without one — the
        // carried block title, the only content in that position — renders its
        // template. Running both and keeping the second would be observable,
        // not merely wasteful: a renderer is a host-supplied trait object, so a
        // stateful one (a recorder, a numbering backend, anything counting its
        // own callbacks) would see every callback for this content twice in one
        // pass.
        //
        // The carve-out this replaces was narrower on paper and wider in fact:
        // it kept the template wherever the tree did not hold *every*
        // cross-reference the string pipeline deferred. Reading the
        // cross-references off the tree makes that condition unstatable — the
        // tree holds all of its own — so what is left is only the content that
        // has no tree at all.
        //
        // A content with deferred cross-references retains its own attributes
        // (`set_render_attributes` is called for exactly that content — see
        // `only_deferred_content_retains_its_render_attributes`), so the fold
        // arm always binds. It is written as a binding rather than an unwrap
        // because the invariant lives in that pairing, not in the field's type.
        if from_tree
            && !self.inlines.is_empty()
            && let Some(attributes) = self.render_attributes.as_deref().cloned()
        {
            self.refold(attributes, renderer, parser);
        } else {
            self.rebuild_rendered(renderer);
        }
    }

    /// Re-renders this content from its **tree**, now that resolution has
    /// installed each cross-reference's destination into it.
    ///
    /// This is what retires the deferred-cross-reference sentinel system
    /// (design §4.2's second). `rendered_html()` became a fold of the tree for
    /// every other content at the cutover; the one exception was content
    /// carrying a deferred cross-reference, whose rendering is rebuilt on every
    /// resolution pass and so could not be a fold *taken at parse time*. It can
    /// be a fold taken **here** instead — after the pass that resolved it —
    /// which is the same answer reached one step later.
    ///
    /// `attributes` are the document attributes this content was parsed under,
    /// which it retained itself because they are order-dependent; they are
    /// paired here with the parser's own configuration, which is not. A content
    /// that retained none has no deferred cross-reference and was already
    /// folded authoritatively at parse time, so it never reaches here.
    ///
    /// **The caller gates this on the mirror having succeeded**, which is the
    /// carve-out that replaces the old one. Until now a content carrying *any*
    /// deferred cross-reference kept the template path end to end; now only one
    /// whose tree does **not** hold every cross-reference the string pipeline
    /// deferred does. That is the single-pass builder's documented set of
    /// unrecognized forms (see the `inline_builder` module) — where one of them
    /// applies, the tree is known not to describe this content, so folding it
    /// would *lose* the construct rather than render it differently. The signal
    /// is the block-level count match
    /// [`mirror_tree_xref_resolution`](Self::mirror_tree_xref_resolution)
    /// already computes, so the carve-out narrows on its own as each builder
    /// prep teaches the builder one more form, and disappears when none is
    /// left.
    ///
    /// This runs **instead of**
    /// [`rebuild_rendered`](Self::rebuild_rendered), not after it. The template
    /// still exists — it is what the other arm renders, and what the test-only
    /// `rendered_from_template` keeps the fold differentiated against — but
    /// rendering both and discarding one would be
    /// observable rather than merely wasteful, since a stateful host renderer
    /// would see every callback for this content twice in a single resolution
    /// pass.
    fn refold(
        &mut self,
        attributes: ResolvedAttributes,
        renderer: &dyn InlineSubstitutionRenderer,
        parser: &Parser,
    ) {
        let context = parser.render_context_with(attributes);
        let folded = crate::content::inline_builder::fold_html(&self.inlines, renderer, &context);

        self.rendered = CowStr::from(folded);
    }

    /// Mirrors the destinations just resolved for this content's deferred
    /// cross-references into its inline tree, so a [`Ref`](InlineNode::Ref)
    /// node of variant [`Xref`](RefVariant::Xref) carries the same
    /// [`resolved`](crate::inlines::Ref::resolved) destination the rendered
    /// string reflects.
    ///
    /// It is a no-op for a content whose tree holds no cross-reference node.
    /// It reuses the results of the deferred-reference
    /// resolution above rather than re-invoking the resolver, correlating each
    /// tree node with its *own* segment **positionally**: the tree's
    /// cross-reference nodes, visited in document order, line up one-to-one
    /// with the block-level deferred segments (those whose placeholder
    /// still appears in the template) in the same order. So node *i* takes
    /// segment *i*'s destination — two references sharing a target but
    /// resolving differently (a custom resolver keying on the per-reference
    /// context) keep their distinct destinations rather than collapsing
    /// onto one. It is non-destructive and re-resolvable, mirroring
    /// [`resolve_references`](Self::resolve_references): each call overwrites
    /// the tree's resolved state from the current deferred results.
    ///
    /// A cross-reference embedded in a **footnote** is carried the same way,
    /// from the complementary list: its segment is re-homed out of the block
    /// template when the footnote's text is extracted, so it is excluded from
    /// the block correlation above and correlated instead with the tree's
    /// footnote subtrees (see [`resolved_destinations`]).
    ///
    /// Returns what
    /// [`mirror_tree_xref_resolution`](Self::mirror_tree_xref_resolution)
    /// returns — whether the tree holds exactly the block-level
    /// cross-references the string pipeline deferred — and `false` for a
    /// content with no tree or nothing deferred, neither of which is re-folded.
    fn resolve_tree_references(&mut self) -> bool {
        let Some(deferred) = self.deferred.as_ref() else {
            return false;
        };

        let (block_ordered, footnote_ordered) = if deferred.from_tree {
            (
                resolved_destinations(&deferred.block),
                resolved_destinations(&deferred.footnote),
            )
        } else {
            // The string pipeline's flat list, split the way it has always been
            // split: a placeholder still in the template is block-level, one
            // that has left it was re-homed onto a footnote. The block half
            // will not correlate (that count mismatch is why this content is on
            // this path at all), but the footnote half still can, and does.
            (
                template_partition(&deferred.template, &deferred.block, true),
                template_partition(&deferred.template, &deferred.block, false),
            )
        };

        let from_tree = deferred.from_tree;

        self.mirror_tree_xref_resolution(&block_ordered, &footnote_ordered);

        from_tree
    }

    /// Installs a pre-computed list of resolved cross-reference destinations —
    /// in placeholder (document) order, as produced by
    /// [`resolved_destinations`] — into this content's inline tree.
    ///
    /// This is the tree-facing half of
    /// [`resolve_references`](Self::resolve_references): where that method
    /// resolves this content's *own* deferred segments and then calls here,
    /// the **document-order title resolution pass** (the `title_refs`
    /// module) resolves a title's cross-references with cross-title
    /// coordination the per-content pass cannot do, and calls here directly
    /// with the destinations it computed. Either way the mirroring is the
    /// same tree walk, so a caller that reads [`inlines`](Self::inlines)
    /// sees the resolved destinations the rendered string reflects.
    ///
    /// `footnote_ordered` carries the same thing for the cross-references
    /// embedded in this content's **footnotes** — the complementary list, as
    /// produced by [`resolved_destinations`] — which are installed into the
    /// tree's footnote subtrees. The two lists partition the deferred segments,
    /// so each is correlated against exactly the nodes it belongs to.
    ///
    /// It is a no-op for a tree holding no cross-reference node,
    /// non-destructive, and re-resolvable: each call overwrites the tree's
    /// resolved state from `block_ordered` and `footnote_ordered`.
    ///
    /// Returns whether the **block-level** correlation ran — i.e. whether the
    /// tree holds exactly the cross-reference nodes whose placeholders the
    /// string pipeline left in the template. A `false` means the builder left
    /// at least one of them unrecognized, so this content's tree is known not
    /// to describe its rendering; a caller that folds the tree instead of
    /// rebuilding from that template (see [`refold`](Self::refold)) uses this
    /// to tell the two apart.
    ///
    /// The **footnote** correlation is deliberately not part of that answer: a
    /// footnote's text is extracted out of this content, so it is not part of
    /// [`rendered`](Self::rendered) — a fold emits the footnote's *marker* and
    /// never descends into its subtree (see `fold_footnote`). A footnote-side
    /// skip therefore leaves the tree's own footnote nodes honestly unresolved
    /// without making a fold of this content wrong.
    pub(crate) fn mirror_tree_xref_resolution(
        &mut self,
        block_ordered: &[Option<ResolvedReference>],
        footnote_ordered: &[Option<ResolvedReference>],
    ) {
        if self.inlines.is_empty() {
            return;
        }

        // The correlation is positional, so it requires the tree to hold
        // exactly one node per segment. The single-pass builder leaves a
        // documented set of divergent forms unrecognized (e.g. a display text
        // crossing a rendered span — see the `inline_builder` module), so the
        // tree can legitimately hold *fewer* cross-reference nodes than the
        // string pipeline deferred. When the counts differ the positional
        // pairing is unknowable; the mirror is skipped for that list — leaving
        // its nodes in their honest unresolved state — rather than assigning
        // destinations to the wrong nodes.
        if count_tree_xrefs(&self.inlines) == block_ordered.len() {
            let mut next = 0;
            assign_tree_xrefs(&mut self.inlines, block_ordered, &mut next);
        }

        if count_footnote_tree_xrefs(&self.inlines) == footnote_ordered.len() {
            let mut next = 0;
            assign_footnote_tree_xrefs(&mut self.inlines, footnote_ordered, &mut next);
        }
    }

    /// Renders this content **from its deferred template**, without installing
    /// the result — the answer [`refold`](Self::refold) replaced, exposed so a
    /// test can still compare the two.
    ///
    /// A deferred content's rendering is now a fold of its tree wherever the
    /// tree is authoritative, so the whole-document parity harness — which
    /// checks a fold against [`rendered_html`](Self::rendered_html) — would
    /// otherwise be comparing the fold against itself for exactly the content
    /// the retirement changed. This is the independent construction it compares
    /// against instead.
    ///
    /// `None` for a content with nothing deferred, which has no template.
    #[cfg(test)]
    pub(crate) fn rendered_from_template(
        &self,
        renderer: &dyn InlineSubstitutionRenderer,
    ) -> Option<String> {
        let deferred = self.deferred.as_ref()?;

        Some(render_template(
            &deferred.template,
            &deferred.block,
            renderer,
        ))
    }

    /// Rebuilds [`Content::rendered`] from the deferred template and the
    /// current (resolved or unresolved) state of its cross-references.
    fn rebuild_rendered(&mut self, renderer: &dyn InlineSubstitutionRenderer) {
        let Some(deferred) = self.deferred.as_ref() else {
            return;
        };

        // A `deferred` content always reaches here with its template captured:
        // `finalize_deferred` sets it at the end of the same `run_pipeline`
        // call that recorded the list, and nothing reads it in between. An
        // empty one here would blank the content, so catch that invariant break
        // in debug builds rather than let it render.
        debug_assert!(!deferred.template.is_empty());

        self.rendered = render_template(&deferred.template, &deferred.block, renderer).into();
    }
}

/// Renders a **title's** tree, with `block_ordered` / `footnote_ordered`
/// installed into a copy of it, or `None` when that tree is not authoritative
/// for the title.
///
/// This is [`Content::refold`] for the one path that cannot use it. A title's
/// rendering is computed by the document-order pass
/// (the `title_refs` module) rather than installed by
/// [`resolve_references`](Content::resolve_references), because a
/// cross-reference between two titles needs coordination the per-content pass
/// cannot do — and that pass needs each title's rendering *while it runs*, as
/// the link text another title's reference splices in. So the fold has to
/// happen there, in place of the template render, rather than after it: taking
/// both would render every deferred title twice through a host renderer that
/// may be counting (see `Content::resolve_references`).
///
/// Hence the copy. The pass holds no `&mut` to the blocks while it computes —
/// it walks them again afterwards to install what it computed — so it folds a
/// clone of the tree and leaves the real one to that later mirror, which
/// installs the same destinations from the same lists.
///
/// `None` is the same carve-out
/// [`mirror_tree_xref_resolution`](Content::mirror_tree_xref_resolution)
/// reports: a tree holding fewer block-level cross-references than the string
/// pipeline deferred does not describe this title, so folding it would drop the
/// construct. The caller renders the template instead.
///
/// Folding renders the **whole** title, not just its cross-references — every
/// styled span, image and special character in it — where the template render
/// this replaces touched only the placeholders. Measured on a heading carrying
/// all three, that is three more renderer callbacks per deferred title (13 → 16
/// for `== A *bold* image:x.png[X] a < b <<t>>`). The calls are duplicates: the
/// string pipeline already made them at parse time.
///
/// That is the transitional cost of an authoritative fold, not a cost this path
/// adds. Every content already pays it — a plain paragraph's `<` is rendered
/// twice today, once by `run_pipeline` and once by the fold that overwrites its
/// answer — and a *deferred paragraph* has paid exactly this since the fold
/// moved to resolution time. A title was the last content still on the cheaper
/// template path, and it was cheaper only because its rendering was less
/// correct. The duplication ends for all of them together, when step 6 takes
/// the string pipeline off the production path; it cannot end here, because
/// folding a tree means rendering it.
///
/// What this *does* avoid is rendering the same title twice within this pass:
/// the fold replaces the template render rather than joining it — see the
/// caller, and `the_title_pass_renders_each_title_once`.
///
/// Only the **block-level** destinations are installed, and the mirror's
/// footnote half has no counterpart here: a fold emits a footnote's *marker*
/// and never descends into its subtree (see `fold_footnote`), so a destination
/// installed there could not reach this string. The real tree still gets both,
/// from the caller's own later mirror — that tree is read by consumers, not
/// just folded.
pub(crate) fn fold_resolved_title(
    inlines: &[InlineNode<'_>],
    block_ordered: &[Option<ResolvedReference>],
    attributes: &ResolvedAttributes,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
) -> Option<String> {
    if inlines.is_empty() || count_tree_xrefs(inlines) != block_ordered.len() {
        return None;
    }

    let mut tree = inlines.to_vec();

    let mut next = 0;
    assign_tree_xrefs(&mut tree, block_ordered, &mut next);

    let context = parser.render_context_with(attributes.clone());

    Some(crate::content::inline_builder::fold_html(
        &tree, renderer, &context,
    ))
}

/// The resolved destinations of `xrefs`, in their own order — the shape
/// [`Content::mirror_tree_xref_resolution`] installs into an inline tree.
///
/// The two lists it takes are this applied to the two a [`DeferredContent`]
/// holds. There is no filtering left to do: the block-level and
/// footnote-embedded references arrive already partitioned, having been read
/// off the tree by two walks that partition them structurally. The string
/// pipeline needed a filter here because it produced one flat list and told the
/// two apart by asking which placeholders its template still spliced.
///
/// A segment that resolved to nothing contributes a `None`, so an unresolved
/// node is left unresolved, exactly as the rendered string leaves it.
pub(crate) fn resolved_destinations(xrefs: &[XrefSegment]) -> Vec<Option<ResolvedReference>> {
    xrefs.iter().map(|xref| xref.resolved.clone()).collect()
}

/// The resolved destinations of the string pipeline's own flat list, filtered
/// to the half `template` still splices (`spliced`) or to the half it no longer
/// does — the block-level and footnote-embedded partitions respectively.
///
/// This is the split the tree performs structurally, done the only way a flat
/// placeholder-indexed list allows. It is reached only for a content the
/// carve-out keeps on the string pipeline's answer — see
/// [`DeferredContent::from_tree`] — and goes with it.
pub(crate) fn template_partition(
    template: &str,
    xrefs: &[XrefSegment],
    spliced: bool,
) -> Vec<Option<ResolvedReference>> {
    xrefs
        .iter()
        .enumerate()
        .filter(|(index, _)| template.contains(&Content::xref_placeholder(*index)) == spliced)
        .map(|(_, xref)| xref.resolved.clone())
        .collect()
}

/// Counts the [`Xref`](RefVariant::Xref) nodes an [`assign_tree_xrefs`] walk
/// over `nodes` would visit — i.e. the block-level cross-reference slots of the
/// tree, excluding footnote subtrees (which [`count_footnote_tree_xrefs`]
/// covers). Used to verify the positional correlation before assigning.
fn count_tree_xrefs(nodes: &[InlineNode<'_>]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            InlineNode::Ref(reference) => {
                usize::from(reference.variant == RefVariant::Xref)
                    + count_tree_xrefs(&reference.children)
            }

            InlineNode::Styled(styled) => count_tree_xrefs(&styled.children),

            // A **visible index term** shows its text in the flow, so the
            // string replacer's own haystack holds any cross-reference written
            // inside it and defers a segment for one. It is the fifth nested
            // node list a tree can hold a construct in (see the side-effect
            // sweep's own note), and the one a walk written by matching on
            // `children` is bound to miss.
            InlineNode::IndexTerm(index_term) => count_tree_xrefs(&index_term.children),

            _ => 0,
        })
        .sum()
}

/// Counts the [`Xref`](RefVariant::Xref) nodes an
/// [`assign_footnote_tree_xrefs`] walk over `nodes` would visit — i.e. the
/// cross-reference slots inside the tree's footnote subtrees.
fn count_footnote_tree_xrefs(nodes: &[InlineNode<'_>]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            InlineNode::Footnote(footnote) => count_tree_xrefs(&footnote.children),

            InlineNode::Ref(reference) => count_footnote_tree_xrefs(&reference.children),

            InlineNode::Styled(styled) => count_footnote_tree_xrefs(&styled.children),

            InlineNode::IndexTerm(index_term) => count_footnote_tree_xrefs(&index_term.children),

            _ => 0,
        })
        .sum()
}

/// Derives the **block-level** deferred cross-reference segments from an
/// already-built inline tree — the list `run_pipeline`'s own
/// [`InlineXrefReplacer`](crate::content::macros) produces and
/// [`Content::set_deferred_xrefs`] installs, read off the nodes instead.
///
/// This is one of the six things design §5.2's survey found the string pipeline
/// still solely owning — the first of them the survey called *blocked* rather
/// than merely unbuilt. It is **wired**: [`Content::set_tree_xrefs`] installs
/// what this returns, so what a content carries for its deferred
/// cross-references is what its tree said — everywhere the tree describes the
/// same cross-references the string pipeline deferred, which is the carve-out
/// [`DeferredContent::from_tree`] names.
///
/// A [`Ref`](InlineNode::Ref)`{`[`Xref`](RefVariant::Xref)`}` node already
/// carries every field an [`XrefSegment`] holds but one — `target`, `window`,
/// `roles`, `xrefstyle` and `derived` are plain values the builder resolved at
/// recognition time — so the survey recorded the family as blocked on
/// [`provided_text`](XrefSegment::provided_text) alone, which the segment holds
/// as a **string** where the node holds its display text as *children*.
///
/// That slot takes the **fold of those children**, and it is a different answer
/// from the one the sibling increment gave `role=` / `window=` / `xrefstyle=`
/// (the author's untranslated source, see
/// [`untranslated_value`](crate::content::inline_builder)) for a reason worth
/// stating: a display text *is* markup by nature. `xref:sec[*bold*]` shows
/// bold text, and the string replacer captures exactly
/// `<strong>bold</strong>` out of its own already-rendered haystack — so the
/// fold reproduces the byte string rather than approximating it, where a
/// computed string slot had no such answer to match.
///
/// The fold runs **here**, at the end of the parse, with the renderer the parse
/// carried — which is where the string replacer computes it too. Deriving it
/// later, at resolution time, would read whatever renderer that caller passed
/// and hand the resolver a different [`ResolutionContext`] than the string
/// pipeline does; taking it at build time keeps the two byte-identical and
/// keeps this function a pure function of the tree plus the parse's own
/// renderer.
///
/// Present-but-empty is preserved, because it is a distinction the renderer
/// acts on: the `<<id,>>` shorthand records one empty
/// [`Text`](InlineNode::Text) child, which the string replacer carries as
/// `Some("")` and renders as an empty `<a>…</a>`, where an absent text
/// (`None`) falls back to the target's reference text. So the `Option` keys on
/// the **presence of a child**, not on what that child folds to — the same rule
/// [`fold_xref`](crate::content::inline_builder) already applies, and the
/// reason `build_xref_shorthand_node` builds a child for a comma-carrying
/// shorthand at all.
///
/// The walk is [`assign_tree_xrefs`]'s, so the order is that walk's order: a
/// pre-order traversal that consumes a slot per cross-reference node and does
/// **not** descend into a [`Footnote`](InlineNode::Footnote) subtree, whose
/// segments are re-homed out of the block template.
/// [`footnote_tree_xref_segments`] derives the complementary list.
pub(crate) fn block_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
) -> Vec<XrefSegment> {
    let mut out = Vec::new();
    collect_tree_xref_segments(nodes, renderer, context, &mut out);
    out
}

/// Derives the deferred cross-reference segments embedded in this tree's
/// **footnote** subtrees — the exact complement of
/// [`block_tree_xref_segments`], in the order
/// [`resolved_destinations`] enumerates them.
///
/// A footnote cannot nest another footnote, so handing each footnote's own
/// children to the block collector cannot skip anything — the same reuse
/// [`assign_footnote_tree_xrefs`] makes.
pub(crate) fn footnote_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
) -> Vec<XrefSegment> {
    let mut out = Vec::new();
    collect_footnote_tree_xref_segments(nodes, renderer, context, &mut out);
    out
}

/// The shared pre-order walk behind [`block_tree_xref_segments`], mirroring
/// [`assign_tree_xrefs`]'s traversal so a derived segment and an installed
/// destination address the same node.
fn collect_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
    out: &mut Vec<XrefSegment>,
) {
    for node in nodes {
        match node {
            InlineNode::Ref(reference) => {
                if reference.variant == RefVariant::Xref {
                    out.push(xref_segment_from_node(reference, renderer, context));
                }

                collect_tree_xref_segments(&reference.children, renderer, context, out);
            }

            InlineNode::Styled(styled) => {
                collect_tree_xref_segments(&styled.children, renderer, context, out);
            }

            InlineNode::IndexTerm(index_term) => {
                collect_tree_xref_segments(&index_term.children, renderer, context, out);
            }

            _ => {}
        }
    }
}

/// The shared walk behind [`footnote_tree_xref_segments`], mirroring
/// [`assign_footnote_tree_xrefs`]'s traversal.
fn collect_footnote_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
    out: &mut Vec<XrefSegment>,
) {
    for node in nodes {
        match node {
            InlineNode::Footnote(footnote) => {
                collect_tree_xref_segments(&footnote.children, renderer, context, out);
            }

            InlineNode::Ref(reference) => {
                collect_footnote_tree_xref_segments(&reference.children, renderer, context, out);
            }

            InlineNode::Styled(styled) => {
                collect_footnote_tree_xref_segments(&styled.children, renderer, context, out);
            }

            InlineNode::IndexTerm(index_term) => {
                collect_footnote_tree_xref_segments(&index_term.children, renderer, context, out);
            }

            _ => {}
        }
    }
}

/// Reads one [`XrefSegment`] off a cross-reference node — see
/// [`block_tree_xref_segments`] for why each field reads the way it does.
///
/// [`resolved`](XrefSegment::resolved) is deliberately **not** carried across:
/// it is resolution's *output*, filled in later by
/// [`Content::resolve_references`] and mirrored back onto the node by
/// [`Content::mirror_tree_xref_resolution`]. A node re-read after a resolution
/// sweep therefore yields the same segment it yielded before one, which is what
/// makes this derivation idempotent.
fn xref_segment_from_node(
    reference: &crate::inlines::Ref<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
) -> XrefSegment {
    let provided_text = (!reference.children.is_empty())
        .then(|| crate::content::inline_builder::fold_html(&reference.children, renderer, context));

    XrefSegment {
        target: reference.target.to_string(),
        provided_text,
        window: reference.window.as_ref().map(|w| w.to_string()),
        roles: reference.roles.iter().map(|r| r.to_string()).collect(),
        xrefstyle: reference.xrefstyle,
        derived: reference.derived.clone(),
        resolved: None,
    }
}

/// Walks an inline node slice in document order and installs each
/// cross-reference's resolved destination from `ordered` — the resolved state
/// of the block-level deferred segments, in placeholder order — advancing
/// `next` past each [`Xref`](RefVariant::Xref) node it visits.
///
/// Only [`Ref`](InlineNode::Ref) nodes of variant [`Xref`](RefVariant::Xref)
/// consume a slot; a [`Link`](RefVariant::Link) has no catalog destination. The
/// pre-order traversal visits cross-references in the same left-to-right order
/// the substitution assigned their placeholders, so node *i* receives segment
/// *i*'s destination — overwritten unconditionally (to `Some` or `None`) so a
/// repeated resolution reflects the latest result. A node with no matching slot
/// (a count mismatch, guarded against by the caller) is left untouched.
///
/// A [`Footnote`](InlineNode::Footnote) node's subtree is deliberately **not**
/// descended into: its cross-references were re-homed onto the footnote and so
/// are absent from `ordered`. Consuming a slot for one would shift every
/// following block-level reference onto the wrong destination. They are
/// installed by [`assign_footnote_tree_xrefs`] from the complementary list.
fn assign_tree_xrefs(
    nodes: &mut [InlineNode<'_>],
    ordered: &[Option<ResolvedReference>],
    next: &mut usize,
) {
    for node in nodes {
        match node {
            InlineNode::Ref(reference) => {
                if reference.variant == RefVariant::Xref {
                    if let Some(resolved) = ordered.get(*next) {
                        reference.resolved = resolved.clone();
                    }

                    *next += 1;
                }

                assign_tree_xrefs(&mut reference.children, ordered, next);
            }

            InlineNode::Styled(styled) => {
                assign_tree_xrefs(&mut styled.children, ordered, next);
            }

            InlineNode::IndexTerm(index_term) => {
                assign_tree_xrefs(&mut index_term.children, ordered, next);
            }

            _ => {}
        }
    }
}

/// Walks an inline node slice in document order and installs each
/// **footnote-embedded** cross-reference's resolved destination from `ordered`
/// — the resolved state of the re-homed deferred segments, in segment order (as
/// produced by [`resolved_destinations`]) — advancing `next` past each one.
///
/// The block walk skips footnote subtrees, so this is the pass that reaches
/// them: for each [`Footnote`](InlineNode::Footnote) node it hands the
/// footnote's own children to [`assign_tree_xrefs`], which assigns them exactly
/// as it assigns block-level references. A footnote cannot nest another
/// footnote, so that reuse cannot skip anything.
fn assign_footnote_tree_xrefs(
    nodes: &mut [InlineNode<'_>],
    ordered: &[Option<ResolvedReference>],
    next: &mut usize,
) {
    for node in nodes {
        match node {
            InlineNode::Footnote(footnote) => {
                assign_tree_xrefs(&mut footnote.children, ordered, next);
            }

            InlineNode::Ref(reference) => {
                assign_footnote_tree_xrefs(&mut reference.children, ordered, next);
            }

            InlineNode::Styled(styled) => {
                assign_footnote_tree_xrefs(&mut styled.children, ordered, next);
            }

            InlineNode::IndexTerm(index_term) => {
                assign_footnote_tree_xrefs(&mut index_term.children, ordered, next);
            }

            _ => {}
        }
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
#[derive(Clone, Eq, Hash, PartialEq)]
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
            passthroughs: Vec::new(),
            inlines: Vec::new(),
            render_attributes: None,
        }
    }
}

// `inlines` is a derived cache (see the field's doc comment), so it is excluded
// from equality and hashing: a `Content` compares and hashes by its rendered
// output and resolution state, whether or not the inline tree was built. This
// preserves the semantics the old `#[derive(Eq, Hash, PartialEq)]` had before
// the field was added.
impl PartialEq for Content<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original
            && self.rendered == other.rendered
            && self.source_lines == other.source_lines
            && self.deferred == other.deferred
            && self.passthroughs == other.passthroughs
    }
}

impl Eq for Content<'_> {}

impl std::hash::Hash for Content<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.original.hash(state);
        self.rendered.hash(state);
        self.source_lines.hash(state);
        self.deferred.hash(state);
        self.passthroughs.hash(state);
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

    mod from_filtered {
        use crate::{Span, content::Content, strings::CowStr};

        #[test]
        fn borrows_source_when_filter_is_a_no_op() {
            // A filtered view byte-identical to the source span is borrowed from
            // the span rather than allocating an owned copy of text we already
            // hold.
            let content = Content::from_filtered(Span::new("plain text"), "plain text");

            assert!(matches!(content.rendered, CowStr::Borrowed(_)));
            assert_eq!(content.rendered.as_ref(), "plain text");
        }

        #[test]
        fn owns_rendered_text_when_filter_changes_it() {
            // A filtered view that differs from the source is materialized as an
            // owned copy.
            let content = Content::from_filtered(Span::new("a|b"), "ab");

            assert!(matches!(content.rendered, CowStr::Boxed(_)));
            assert_eq!(content.rendered.as_ref(), "ab");
        }
    }

    mod sanitize_title {
        use super::super::sanitize_title;

        #[test]
        fn leaves_plain_text_unchanged() {
            assert_eq!(sanitize_title("Plain title"), "Plain title");
        }

        #[test]
        fn strips_a_tag_pair_keeping_its_inner_text() {
            assert_eq!(sanitize_title("<strong>bold</strong>"), "bold");
        }

        #[test]
        fn strips_a_self_contained_tag() {
            assert_eq!(
                sanitize_title(r#"Before <img src="a.png" alt="a"> after"#),
                "Before after",
            );
        }

        #[test]
        fn squeezes_spaces_left_behind_by_a_removed_tag() {
            // A run of spaces left behind after tags are stripped (e.g. two
            // images rendered back-to-back, or a tag flanked by spaces on
            // both sides) collapses to one, and the ends are trimmed,
            // mirroring Ruby's `tr_s(' ', ' ').strip`.
            assert_eq!(
                sanitize_title(r#"<img src="a.png">  <img src="b.png">"#),
                "",
            );
            assert_eq!(
                sanitize_title("Before <b>bold</b>   after"),
                "Before bold after",
            );
        }

        #[test]
        fn an_empty_tag_does_not_match_and_is_kept_verbatim() {
            // `[^>]+` in Ruby's `XmlSanitizeRx` requires at least one
            // character between `<` and `>`; an empty `<>` does not match
            // and is copied through as literal text.
            assert_eq!(sanitize_title("a<>b"), "a<>b");
        }

        #[test]
        fn an_unclosed_angle_bracket_is_kept_verbatim() {
            // A `<` with no `>` anywhere after it in the rest of the string
            // is not a tag and is left as literal text, along with
            // everything after it.
            assert_eq!(
                sanitize_title("Title <dangling and more"),
                "Title <dangling and more",
            );
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
