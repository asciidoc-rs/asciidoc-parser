//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use crate::{
    Parser, Span,
    content::Passthrough,
    inlines::{InlineNode, Ref, RefVariant},
    parser::{
        InlineRenderer, ReferenceResolver, ReferenceWarnings, RenderContext, ResolutionContext,
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
    /// inline nodes, built by the single-pass builder (the crate-internal
    /// `inline_builder` module) directly from the pre-substitution source.
    ///
    /// This tree is **authoritative**: [`rendered`](Self::rendered) is a fold
    /// of it (see `SubstitutionGroup::apply`), and every macro family's
    /// catalog/warning registration replays from it. It is nonetheless
    /// deliberately excluded from [`PartialEq`]/[`Eq`]/[`Hash`], as a derived
    /// artifact: two `Content`s with equal rendered text compare equal
    /// regardless of their trees.
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
    /// # Why the attributes and not a whole [`RenderContext`]
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
/// [`block_tree_xref_segments`] and [`footnote_tree_xref_segments`]), and
/// arrive already partitioned into the two lists resolution keeps apart —
/// rather than a flat list that has to be split by asking which placeholder
/// survived.
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

    /// The deferred template: the content's rendering as a list of
    /// [`XrefTemplatePiece`]s — literal runs of already-rendered text,
    /// interleaved with the positions of the cross-references to splice in
    /// between them.
    ///
    /// This used to be one `String` in *escaped sentinel form*: a raw
    /// `U+E000 <index> U+E001` marked each splice point, and every
    /// document-derived byte was escaped so a typed copy of that sequence
    /// could not forge one (see `escape_sentinels`, since retired). The
    /// structure makes both halves of that machinery unnecessary — a splice
    /// point is a [`Xref`](XrefTemplatePiece::Xref) variant rather than a
    /// byte pattern, so nothing scans the literal text and nothing in it
    /// needs escaping.
    ///
    /// The one content that *renders* from a template in production — a block
    /// title carried across a section heading, which travels through
    /// `Parser::pending_block_title` as an [`OwnedTitle`] because the parser it
    /// rides on has no `'src` lifetime, and so arrives at the claiming block
    /// with its inline nodes dropped — carries a template synthesized **from
    /// its own tree** at the hop (see [`Content::to_owned_title`] and
    /// [`carried_title_template`]). Its rendering cannot be a fold, so it
    /// stays a splice — the document-order title pass renders it through
    /// [`render_xref_template`].
    ///
    /// Empty for every production content but the carried title above.
    template: Vec<XrefTemplatePiece>,
}

/// A read-only view of a [`Content`]'s deferred cross-references — the shape
/// [`Content::deferred_parts`] hands out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DeferredParts<'a> {
    /// The block-level cross-references, in document order.
    pub(crate) block: &'a [XrefSegment],

    /// The cross-references this content's footnotes carry.
    pub(crate) footnote: &'a [XrefSegment],

    /// The deferred template, for a content that renders from one — see
    /// [`DeferredContent::template`].
    pub(crate) template: &'a [XrefTemplatePiece],
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

/// One piece of a [`DeferredContent::template`]: a literal run of
/// already-rendered text, or the position of a cross-reference to splice.
///
/// This is the **out-of-band** replacement for the in-band placeholder
/// sentinels (`U+E000 <index> U+E001`) the template used to be written in.
/// In-band marks needed the escaping pass — `escape_sentinels`, since retired
/// — to stay unforgeable — a document-typed copy of the byte sequence was
/// otherwise byte-identical to a real placeholder. A splice point that is a
/// variant rather than a byte pattern cannot be typed at all: a document's own
/// `U+E000 0 U+E001` is content inside a [`Literal`](Self::Literal), and
/// nothing reads placeholders back out of text.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum XrefTemplatePiece {
    /// Already-rendered literal text, emitted verbatim by the splice.
    Literal(String),

    /// The cross-reference at this index in the template's segment list,
    /// rendered from the segment's current (resolved or unresolved) state at
    /// splice time.
    Xref(usize),
}

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
    pub(super) fn set_render_attributes(&mut self, attributes: ResolvedAttributes) {
        self.render_attributes = Some(Box::new(attributes));
    }

    /// Returns a fully-owned snapshot of this content's rendered text and
    /// deferred cross-references, for a title that must outlive its source
    /// borrow (see [`OwnedTitle`]).
    ///
    /// A title that defers a cross-reference and still holds its tree swaps its
    /// deferred template — and the segment list the template's pieces point
    /// into — for the **tree's own**, synthesized here by
    /// [`carried_title_template`]. The snapshot travels across the
    /// `'src`-erasing hop (`Parser::pending_block_title`) with its inline nodes
    /// dropped, so the claiming block's title is the one content that renders
    /// from a template rather than folding a tree — and that template is a
    /// product of the tree, synthesized at this hop.
    ///
    /// `parser` supplies the renderer; the fold runs under the title's own
    /// retained render attributes, the same pairing [`refold`](Self::refold)
    /// uses. A deferring content always retains them
    /// (`set_render_attributes` is called for exactly that content), so the
    /// binding below always takes for a title with a tree; it is written as a
    /// binding rather than an unwrap because the invariant lives in that
    /// pairing, not in the field's type. The condition's other exit is real:
    /// a title **re-stashed** past an empty section body arrives here with its
    /// nodes already dropped and keeps the template the first hop synthesized.
    pub(crate) fn to_owned_title(&self, parser: &Parser) -> OwnedTitle {
        let deferred = self.deferred.as_ref().map(|d| {
            let mut d = (**d).clone();

            if !self.inlines.is_empty()
                && let Some(attributes) = self.render_attributes.as_deref()
            {
                let context = parser.render_context_with(attributes.clone());

                let (template, block) =
                    carried_title_template(&self.inlines, &*parser.renderer, &context);

                d.template = template;
                d.block = block;
            }

            d
        });

        OwnedTitle {
            rendered: self.rendered.as_ref().to_string(),
            deferred,
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
            deferred: title.deferred.map(Box::new),
            passthroughs: Vec::new(),
            inlines: Vec::new(),
            render_attributes: title.render_attributes,
        }
    }

    /// Constructs a `Content` from a source `Span` and the per-line filtered
    /// view of that source.
    pub(crate) fn from_filtered_lines(span: Span<'src>, filtered_lines: &[&'src str]) -> Self {
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

    /// Returns the default **HTML** rendering of this content: the final text
    /// after all substitutions have been applied.
    ///
    /// This is the built-in HTML output. A custom [`InlineRenderer`]
    /// installed via
    /// [`Parser::with_inline_renderer`](crate::Parser::with_inline_renderer)
    /// drives this output too; [`render_with`](Self::render_with) folds the
    /// same tree through any renderer supplied at render time instead.
    pub fn rendered_html(&'src self) -> &'src str {
        self.rendered.as_ref()
    }

    /// Renders this content to a caller-supplied backend: a pure fold over the
    /// same [`inlines`](Self::inlines) tree
    /// [`rendered_html`](Self::rendered_html) is the built-in HTML answer for.
    /// Returns an owned `String`, and caches nothing — the crate memoizes only
    /// the default HTML rendering.
    ///
    /// One parse feeds any number of renders: the tree is built once, with
    /// every order-dependent fact (footnote numbers, counters, expanded
    /// attribute values, resolved cross-reference destinations) already
    /// resolved into node values, so a render is a pure
    /// `(tree, renderer, context) → String` with no document traversal left to
    /// do.
    ///
    /// # Why this takes `parser`
    ///
    /// A fold needs a [`RenderContext`], which
    /// pairs two things from different places. The **document attributes** are
    /// order-dependent — a `:imagesdir:` or `:icons:` line rebinds them for
    /// everything after it — so they must be the ones *this content* was
    /// parsed under, and the content retains them for exactly that reason. The
    /// **path resolver and file handlers** are parse-wide configuration that
    /// cannot change mid-parse, so they are not frozen per content: they are
    /// `Rc<dyn …>`, and holding them here would cost [`Content`] — and with it
    /// [`Document`](crate::Document) — its [`Send`]/[`Sync`]. Supplying the
    /// parser the caller already holds is the cheaper half of that trade.
    ///
    /// # Which parser to supply
    ///
    /// The one that parsed this content. Nothing in the type system ties the
    /// two together — a `Content` holds no reference to its parser, which is
    /// what keeps it [`Send`]/[`Sync`] — so this is a contract rather than a
    /// guarantee, and it is worth being precise about what a caller risks by
    /// breaking it.
    ///
    /// The **document attributes always come from this content's own
    /// snapshot**, never from `parser`. Supplying a different parser therefore
    /// cannot change how a construct is rendered as a function of document
    /// state: `icons`, `data-uri` and the safe mode are read from the values
    /// in effect where this content was written, whichever parser is handed
    /// in (`render_with_takes_document_attributes_from_the_content` pins
    /// this). What *does* follow `parser` is its parse-wide configuration —
    /// the path resolver and the image/SVG file handlers — so a mismatched
    /// parser resolves image paths and reads embedded files the way *it* was
    /// configured to.
    ///
    /// # Content with no tree
    ///
    /// A content whose substitution group is never applied at all — a
    /// `[comment]`-styled paragraph, whose text is retained but deliberately
    /// not interpreted — carries no inline tree and no retained attributes.
    /// There is nothing to fold, so its literal text is returned unchanged,
    /// which is also what [`rendered_html`](Self::rendered_html) gives.
    pub fn render_with(&self, renderer: &dyn InlineRenderer, parser: &Parser) -> String {
        let Some(attributes) = self.render_attributes.as_deref() else {
            return self.rendered.to_string();
        };

        crate::content::inline_builder::fold_html(
            &self.inlines,
            renderer,
            &parser.render_context_with(attributes.clone()),
        )
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
    pub(super) fn set_passthroughs(&mut self, passthroughs: Vec<Passthrough>) {
        self.passthroughs = passthroughs;
    }

    /// Returns the inline AST for this content: the structured, read-only
    /// representation of its inline nodes.
    ///
    /// Every parse builds this, so it is an empty slice only for content
    /// whose tree is genuinely empty (empty content). The tree is built by
    /// the single-pass builder (the crate-internal `inline_builder` module)
    /// directly from the pre-substitution source, so each node carries its
    /// own precise source [`Span`] (a node born from a transformation, such
    /// as an attribute expansion, falls back to a documented coarser span)
    /// and a macro node carries its own parsed attribute list. The tree is
    /// **canonical**: [`rendered_html`](Self::rendered_html) is a fold of
    /// it. A small set of forms is documented as deferred (in the
    /// `inline_builder` module), each left as literal, already-rendered text
    /// in the tree rather than a dedicated node — so the rendering is always
    /// exact, but the tree's structure is coarser for those forms.
    ///
    /// Cross-references in the tree carry their resolved destination once a
    /// full [`Parser::parse`](crate::Parser::parse) has resolved the document's
    /// references: each resolved destination is mirrored into the corresponding
    /// [`Ref`] node, so a caller that walks
    /// [`inlines`](Self::inlines) after the parse sees the same destinations
    /// the rendered string reflects. Before resolution — or for a standalone
    /// parse with no document catalog — a `Ref` node's destination is `None`.
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
    /// The template is the [`XrefTemplatePiece`] list — synthesized from the
    /// tree by [`carried_title_template`] for the one production content that
    /// renders from one; the segments are the
    /// cross-references in splice order. Used by the document-order title
    /// resolution pass, which re-renders a title's cross-references with
    /// cross-title (including circular) coordination that the per-content
    /// [`resolve_references`](Self::resolve_references) cannot provide.
    pub(crate) fn deferred_parts(&self) -> Option<DeferredParts<'_>> {
        self.deferred.as_ref().map(|d| DeferredParts {
            block: &d.block,
            footnote: &d.footnote,
            template: &d.template,
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

    /// Installs this content's deferred cross-references, **read off its own
    /// inline tree** — the sole producer of a content's deferred state.
    ///
    /// The two lists come from [`block_tree_xref_segments`] and
    /// [`footnote_tree_xref_segments`], already partitioned the way resolution
    /// keeps them apart. There is no template here: the one content that
    /// renders from a template — the carried block title — has its own
    /// synthesized at the hop (see [`carried_title_template`]), and no other
    /// reader consults [`DeferredContent::template`] on a tree content.
    ///
    /// A tree holding no cross-reference installs nothing: such a content
    /// renders as the fold leaves it and resolution has nothing to do. The
    /// boolean walk answers that first, and cheaply, because *most contents
    /// are exactly that* — a plain paragraph's nodes are text runs the walk
    /// rejects in one pass, where unconditionally deriving both segment lists
    /// would traverse every tree twice to build two empty vectors.
    ///
    /// A cross-reference form the builder declines to recognize is simply not
    /// a cross-reference of this content — the documented-divergence reading
    /// every such form already has.
    pub(super) fn set_tree_xrefs(
        &mut self,
        tree: &[InlineNode<'src>],
        renderer: &dyn InlineRenderer,
        context: &RenderContext,
    ) {
        if !tree_defers_xrefs(tree) {
            return;
        }

        debug_assert!(
            self.deferred.is_none(),
            "set_tree_xrefs must be called at most once per Content"
        );

        let block = block_tree_xref_segments(tree, renderer, context);
        let footnote = footnote_tree_xref_segments(tree, renderer, context);

        self.deferred = Some(Box::new(DeferredContent {
            block,
            footnote,
            template: Vec::new(),
        }));
    }

    /// Resolves any deferred cross-references using `resolver`, then rebuilds
    /// the rendered text.
    ///
    /// This is non-destructive: the deferred template is retained, so a
    /// document may be resolved more than once (e.g. for incremental builds or
    /// multiple output targets).
    ///
    /// Any target that the resolver cannot resolve is reported in `warnings`.
    pub(crate) fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineRenderer,
        warnings: &mut ReferenceWarnings<'src>,
        parser: &Parser,
    ) {
        let source = self.original;

        if let Some(deferred) = self.deferred.as_mut() {
            // The two lists arrive off the tree already partitioned, and only
            // the block-level ones are reported here: a footnote's own copy of
            // an embedded reference resolves and reports it.
            for xref in deferred.block.iter_mut() {
                // The catalog holds the document's own text (an ID as it was
                // written, a section's reference text), which is exactly what
                // a tree-read segment carries.
                let resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });

                // A target that names a document is never reported: it carries
                // its own destination, so there was nothing here to resolve.
                if resolved.is_none() && xref.derived.is_none() {
                    warnings.unresolved(&xref.target, source);
                }

                xref.resolved = resolved;
            }

            for xref in deferred.footnote.iter_mut() {
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &xref.target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });
            }
        }

        let resolved_tree = self.resolve_tree_references();

        // Independent of the arm chosen below. A content whose *only*
        // cross-references sit inside its footnotes defers nothing itself — the
        // replacer captures those onto the footnote's own state — so it reaches
        // here with no deferred cross-references and takes the template arm,
        // while its footnotes are exactly the ones needing a fresh rendering.
        // Gating the fold on this content's own deferral would therefore miss
        // the footnotes that most need it.
        self.collect_own_folded_footnotes(renderer, parser, warnings);

        // A deferring content is re-rendered by folding its tree; a content
        // with nothing deferred keeps the rendering it has. The one content
        // that renders from a *template* — a block title carried across a
        // section heading, whose inline nodes cannot cross the `'src`-erasing
        // hop — never reaches this per-content pass at all: titles are
        // resolved by the document-order title pass
        // (`title_refs::resolve_title_references`), which splices its
        // template through `render_xref_template`.
        //
        // A content with deferred cross-references always has its tree
        // installed and retains its own attributes (`set_render_attributes`
        // is called for exactly that content — see
        // `only_deferred_content_retains_its_render_attributes`), so the
        // bindings below always take with `resolved_tree`; they are written
        // as bindings rather than unwraps because the invariant lives in that
        // pairing, not in the fields' types.
        if resolved_tree
            && !self.inlines.is_empty()
            && let Some(attributes) = self.render_attributes.as_deref().cloned()
        {
            self.refold(attributes, renderer, parser);
        }
    }

    /// Re-renders this content from its **tree**, now that resolution has
    /// installed each cross-reference's destination into it.
    ///
    /// `rendered_html()` is a fold of the tree for every content, taken once
    /// at parse time. The one exception is content carrying a deferred
    /// cross-reference: its rendering must be rebuilt on every resolution
    /// pass, so it cannot be folded once and for all at parse time — what it
    /// holds until resolution is the fold of the still-unresolved tree. This
    /// re-fold, taken **here** after the pass that resolved it, is the same
    /// answer reached one step later.
    ///
    /// `attributes` are the document attributes this content was parsed under,
    /// which it retained itself because they are order-dependent; they are
    /// paired here with the parser's own configuration, which is not. A content
    /// that retained none has no deferred cross-reference and was already
    /// folded authoritatively at parse time, so it never reaches here.
    ///
    /// The caller gates this on the content **holding a tree**, which every
    /// production content with deferred state does — the deferred lists are
    /// read off that tree, the sole producer of a content's deferred state.
    /// The one content that renders from a *template* instead — the carried
    /// block title (see [`carried_title_template`]) — is resolved by the
    /// document-order title pass, never here.
    /// Folds each footnote this content **defines** from its own subtree, and
    /// hands the results to `warnings` for the resolution pass to install into
    /// the catalog.
    ///
    /// This is [`refold`](Self::refold) for the footnote catalog. A footnote's
    /// entry is re-rendered on every resolution pass, for the same reason a
    /// deferred content is: its text may embed a cross-reference whose
    /// destination is only known once resolution has run. The entry could not
    /// hold a tree to fold — it outlives the parse borrow, and
    /// [`InlineNode`] carries a [`Span`] — so it held a
    /// placeholder template instead. It does not need to hold one: **the tree
    /// is already here.** The defining [`Footnote`](crate::inlines::Footnote)
    /// node in this content's own tree carries the footnote's children, and by
    /// this point they carry the destinations just resolved —
    /// [`resolve_tree_references`](Self::resolve_tree_references) installs a
    /// footnote's embedded cross-references into its subtree alongside the
    /// block-level ones.
    ///
    /// So the fold happens where the tree is, and only the resulting `String`
    /// travels to the catalog. Nothing gains a lifetime.
    ///
    /// Only *defining* occurrences are folded. A bare reference to an existing
    /// footnote carries no children and re-uses the defining entry, so folding
    /// it would install an empty rendering over a real one.
    ///
    /// The trim-and-collapse mirrors `register_footnote_number`'s
    /// normalization of the template it registers, so the two producers agree
    /// byte for byte. The `\]` unescape that normalization also performs is
    /// deliberately absent: the subtree's text left that form when it was
    /// built (see `footnote_children`), and applying it again would unescape a
    /// string that was never escaped.
    /// Folds this content's defining footnotes under **its own** retained
    /// render attributes, when it has any.
    ///
    /// Two passes resolve content and so two passes have to collect: the
    /// per-content one below, and the document-order title pass
    /// (`document::title_refs`), which owns section headings and block titles
    /// and does not route them through
    /// [`resolve_references`](Self::resolve_references) at all. Both want the
    /// same thing of the same content, so they ask for it the same way rather
    /// than each re-deriving "which attributes, and is there anything to fold".
    ///
    /// A content with no retained attributes folds nothing: it is one whose
    /// rendering is never rebuilt after the parse, so no footnote of its can
    /// need a fresh one either (see `SubstitutionGroup::apply`, which retains
    /// them for exactly the contents that are rebuilt).
    pub(crate) fn collect_own_folded_footnotes(
        &self,
        renderer: &dyn InlineRenderer,
        parser: &Parser,
        warnings: &mut ReferenceWarnings<'src>,
    ) {
        if let Some(attributes) = self.render_attributes.as_deref() {
            self.collect_folded_footnotes(attributes, renderer, parser, warnings);
        }
    }

    fn collect_folded_footnotes(
        &self,
        attributes: &ResolvedAttributes,
        renderer: &dyn InlineRenderer,
        parser: &Parser,
        warnings: &mut ReferenceWarnings<'src>,
    ) {
        let mut found = vec![];
        defining_footnotes(&self.inlines, &mut found);

        if found.is_empty() {
            return;
        }

        let context = parser.render_context_with(attributes.clone());

        for (index, children) in found {
            let rendered = crate::content::inline_builder::fold_html(children, renderer, &context);

            warnings
                .footnote_texts
                .push((index.to_string(), rendered.trim().replace('\n', " ")));
        }
    }

    fn refold(
        &mut self,
        attributes: ResolvedAttributes,
        renderer: &dyn InlineRenderer,
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
    /// Returns whether there was deferred state to install — `false` for a
    /// content with nothing deferred, which is not re-folded.
    fn resolve_tree_references(&mut self) -> bool {
        let Some(deferred) = self.deferred.as_ref() else {
            return false;
        };

        let block_ordered = resolved_destinations(&deferred.block);
        let footnote_ordered = resolved_destinations(&deferred.footnote);

        self.mirror_tree_xref_resolution(&block_ordered, &footnote_ordered);

        true
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
    /// tree holds exactly the cross-reference nodes the carried title's own
    /// template expects a slot for. A `false` means the builder left
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
        // template expects a slot for. When the counts differ the positional
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
/// reports: a tree holding fewer block-level cross-references than were
/// deferred does not describe this title, so folding it would drop the
/// construct. The caller renders the template instead — the fallback for a
/// title with no tree to fold at all (a block title carried across a section
/// heading, whose inline nodes cannot cross the `'src`-erasing hop it
/// travels on).
///
/// Folding renders the **whole** title, not just its cross-references — every
/// styled span, image and special character in it — where the template render
/// touches only the placeholders. Measured on a heading carrying all three,
/// that is three more renderer callbacks per deferred title (13 → 16 for
/// `== A *bold* image:x.png[X] a < b <<t>>`) — the cost of an authoritative
/// fold, paid once per deferred title.
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
    renderer: &dyn InlineRenderer,
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

            // A **visible index term** shows its text in the flow, so a
            // cross-reference written inside it renders in place and defers a
            // segment of its own. It is the fifth nested
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

/// Whether `nodes` holds any cross-reference node at all — block-level or
/// inside a footnote's subtree — i.e. whether [`Content::set_tree_xrefs`] has
/// anything to install.
///
/// A short-circuiting boolean walk, so the overwhelmingly common
/// cross-reference-free content (a plain paragraph) answers in one cheap pass
/// instead of paying the two full segment derivations to learn both come back
/// empty.
fn tree_defers_xrefs(nodes: &[InlineNode<'_>]) -> bool {
    nodes.iter().any(|node| match node {
        InlineNode::Ref(reference) => {
            reference.variant == RefVariant::Xref || tree_defers_xrefs(&reference.children)
        }

        InlineNode::Styled(styled) => tree_defers_xrefs(&styled.children),

        InlineNode::IndexTerm(index_term) => tree_defers_xrefs(&index_term.children),

        InlineNode::Footnote(footnote) => tree_defers_xrefs(&footnote.children),

        _ => false,
    })
}

/// Derives the **block-level** deferred cross-reference segments from an
/// already-built inline tree, read off the nodes.
///
/// [`Content::set_tree_xrefs`] installs
/// what this returns, so what a content carries for its deferred
/// cross-references is what its tree said.
///
/// A [`Ref`](InlineNode::Ref)`{`[`Xref`](RefVariant::Xref)`}` node already
/// carries every field an [`XrefSegment`] holds but one — `target`, `window`,
/// `roles`, `xrefstyle` and `derived` are plain values the builder resolved at
/// recognition time. Only
/// [`provided_text`](XrefSegment::provided_text) needs deriving, which the
/// segment holds as a **string** where the node holds its display text as
/// *children*.
///
/// That slot takes the **fold of those children**, and it is a different answer
/// from the one given `role=` / `window=` / `xrefstyle=`
/// (the author's untranslated source, see
/// [`untranslated_value`](crate::content::inline_builder)) for a reason worth
/// stating: a display text *is* markup by nature. `xref:sec[*bold*]` shows
/// bold text, and the fold of the node's own children reproduces exactly
/// `<strong>bold</strong>` — the byte string a display text always renders as,
/// rather than approximating it, where a computed string slot had no such
/// answer to match.
///
/// The fold runs **here**, at the end of the parse, with the renderer the parse
/// carried. Deriving it
/// later, at resolution time, would read whatever renderer that caller passed
/// and hand the resolver a different [`ResolutionContext`]; taking it at
/// build time keeps this function a pure function of the tree plus the parse's
/// own renderer.
///
/// Present-but-empty is preserved, because it is a distinction the renderer
/// acts on: the `<<id,>>` shorthand records one empty
/// [`Text`](InlineNode::Text) child, folding to
/// `Some("")` and rendering as an empty `<a>…</a>`, where an absent text
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
fn block_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
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
fn footnote_tree_xref_segments(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
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
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
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
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
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
/// Shared with the placeholder-emitting fold
/// ([`fold_deferring_xrefs`](crate::content::inline_builder::fold_deferring_xrefs)),
/// which captures a footnote's own segments as it writes that footnote's
/// template: both readings of "what does this node defer?" are this one
/// function, so the block-level list and a footnote's own cannot drift.
///
/// [`resolved`](XrefSegment::resolved) is deliberately **not** carried across:
/// it is resolution's *output*, filled in later by
/// [`Content::resolve_references`] and mirrored back onto the node by
/// [`Content::mirror_tree_xref_resolution`]. A node re-read after a resolution
/// sweep therefore yields the same segment it yielded before one, which is what
/// makes this derivation idempotent.
pub(super) fn xref_segment_from_node(
    reference: &Ref<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
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

/// Synthesizes the deferred template — and the segment list its
/// [`Xref`](XrefTemplatePiece::Xref) pieces index into — that a **carried
/// block title** takes across the `'src`-erasing hop (see
/// [`Content::to_owned_title`]), from the title's own inline tree.
///
/// The construction is a walk over the tree's **top-level** nodes: a
/// cross-reference node contributes an [`Xref`](XrefTemplatePiece::Xref) piece
/// and its [`XrefSegment`] (via [`xref_segment_from_node`], the same reading
/// every other segment derivation uses), and every other node contributes its
/// own fold to a [`Literal`](XrefTemplatePiece::Literal) piece. The structure
/// is what keeps the template's two populations apart — a splice point is a
/// variant, not a byte pattern — where the string-form template this replaces
/// had to write in-band `U+E000 <index> U+E001` sentinels and pass every
/// document-derived byte through `escape_sentinels` (since retired) so a
/// document-typed copy of that sequence (see the `sentinels` test module, issue
/// #1235) could not forge a placeholder. Nothing here scans text, so nothing
/// needs escaping.
///
/// The price of reading only the top level is a cross-reference **nested**
/// inside another top-level construct (a styled span's children, a visible
/// index term's shown text): its enclosing node folds as one literal, so the
/// reference is baked into the template as its unresolved fallback rendering
/// rather than spliced — and, having no segment, it is neither resolved nor
/// reported by the title pass. The string pipeline's template did splice those
/// (its placeholders sat inside the rendered markup). That narrowing is
/// accepted for the same reason the carried title renders from a template at
/// all — no tree survives the hop to do better — and it is measured at zero:
/// no golden source carries a nested cross-reference in a carried block title.
/// `a_reference_nested_in_a_span_of_a_carried_title_stays_its_fallback` pins
/// the boundary.
fn carried_title_template(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
) -> (Vec<XrefTemplatePiece>, Vec<XrefSegment>) {
    let mut template: Vec<XrefTemplatePiece> = Vec::new();
    let mut segments = Vec::new();

    for node in nodes {
        match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => {
                let index = segments.len();
                segments.push(xref_segment_from_node(reference, renderer, context));
                template.push(XrefTemplatePiece::Xref(index));
            }

            other => {
                let folded = crate::content::inline_builder::fold_html(
                    std::slice::from_ref(other),
                    renderer,
                    context,
                );

                // Adjacent non-reference nodes coalesce into one literal run,
                // so the piece list mirrors the splice structure rather than
                // the node count.
                match template.last_mut() {
                    Some(XrefTemplatePiece::Literal(text)) => text.push_str(&folded),
                    _ => template.push(XrefTemplatePiece::Literal(folded)),
                }
            }
        }
    }

    (template, segments)
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

/// Splices resolved (or fallback) cross-reference renderings into a
/// structured [`XrefTemplatePiece`] template, producing the final rendered
/// text.
///
/// This is the seam used by the document-order title resolution pass: it hands
/// in a title's captured template together with a set of [`XrefSegment`]s whose
/// [`resolved`](XrefSegment::resolved) fields it has filled in with cross-title
/// (including circular) coordination, and receives the final rendered title.
///
/// A [`Literal`](XrefTemplatePiece::Literal) is emitted verbatim: the pieces
/// carry already-rendered text in the document's own bytes (never in escaped
/// sentinel form — see [`carried_title_template`]), so there is nothing to
/// scan for and nothing to decode. An [`Xref`](XrefTemplatePiece::Xref) piece
/// whose index names no segment renders nothing; the synthesis assigns each
/// index from the position in the very list it travels with, so no producer
/// can write one (`an_xref_piece_past_the_segment_list_renders_nothing` pins
/// the behavior).
pub(crate) fn render_xref_template(
    template: &[XrefTemplatePiece],
    xrefs: &[XrefSegment],
    renderer: &dyn InlineRenderer,
) -> String {
    let mut out = String::new();

    for piece in template {
        match piece {
            XrefTemplatePiece::Literal(text) => out.push_str(text),

            XrefTemplatePiece::Xref(index) => {
                if let Some(xref) = xrefs.get(*index) {
                    render_xref_segment(xref, renderer, &mut out);
                }
            }
        }
    }

    out
}

/// Renders one deferred cross-reference from its segment's current (resolved
/// or unresolved) state, through the same `render_xref` a fold feeds.
///
/// A segment's fields are the tree's reads — the document's own text, never in
/// escaped form; `derived` and `resolved` never entered it either.
///
/// `pub(super)` rather than private: [`fold_deferring_xrefs`]'s own deferred
/// fold renders a **nested** cross-reference's unresolved fallback in place
/// through this same function, so a footnote's baked-literal reading and
/// [`render_xref_template`]'s spliced one can never drift apart.
///
/// [`fold_deferring_xrefs`]: crate::content::inline_builder::fold_deferring_xrefs
pub(super) fn render_xref_segment(
    xref: &XrefSegment,
    renderer: &dyn InlineRenderer,
    out: &mut String,
) {
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
        out,
    );
}

/// The deferred cross-references carried by a footnote's text.
///
/// A footnote's text is extracted out of the flow of the block during the
/// macros substitution step, so any cross-reference (`<<id>>`, `xref:id[…]`)
/// inside it cannot be resolved by the document-level pass that resolves
/// references in block content. Instead, the footnote captures its
/// cross-references here — as a structured template plus the references in
/// placeholder order — and they are resolved alongside the block references
/// (see [`Footnote::resolve_references`]).
///
/// # Every cross-reference is recorded; not every one is a splice point
///
/// [`xrefs`](Self::xrefs) holds **every** cross-reference the footnote's text
/// carries — top-level and nested alike, in document order — because
/// [`resolve`](Self::resolve)'s warnings have to cover both: a footnote's
/// embedded cross-references are never independently warned about anywhere
/// else (`Content::resolve_references` resolves the complementary list it
/// reads off the *enclosing* content's tree but deliberately does not warn
/// on it, leaving that to this type alone). [`template`](Self::template),
/// by contrast, can only splice a **top-level** reference — one directly
/// among the footnote's own children — because a nested one's placeholder
/// would sit *inside* a sibling piece's already-rendered markup (a styled
/// span's body, a link's display text), which a flat piece list cannot
/// represent; see
/// [`fold_deferring_xrefs`](crate::content::inline_builder::fold_deferring_xrefs)
/// for how the two lists diverge in length for such a footnote. A nested
/// reference is still in `xrefs` — still resolved, still warned about if
/// unresolvable — it is simply baked into its enclosing piece as its
/// unresolved-fallback rendering rather than addressable by index.
///
/// [`Footnote::resolve_references`]: crate::document::Footnote::resolve_references
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct FootnoteDeferred {
    /// The footnote's structured template: literal runs of already-rendered
    /// text interleaved with the positions of its **top-level**
    /// cross-references — see the type's own docs for why only those are
    /// addressable.
    template: Vec<XrefTemplatePiece>,

    /// Every cross-reference the footnote's text carries, in document order
    /// — see the type's own docs for why this is not only the ones
    /// `template` can splice.
    xrefs: Vec<XrefSegment>,
}

impl FootnoteDeferred {
    /// Constructs a footnote's deferred cross-reference state from its
    /// `template` and `xrefs` (in placeholder order).
    pub(crate) fn new(template: Vec<XrefTemplatePiece>, xrefs: Vec<XrefSegment>) -> Self {
        Self { template, xrefs }
    }

    /// Every cross-reference this footnote's text carries, in document order
    /// — top-level and nested alike; see the type's own docs. Exposed for the
    /// `inline_builder_side_effect_parity` corpus, which compares this list
    /// on its own rather than [`FootnoteDeferred`]'s whole `Debug` spelling —
    /// [`template`](Self::template)'s shape is no longer something the
    /// string-pipeline-recorded golden can spell, so its literal bytes are
    /// left to the entry's already-compared `text` field instead.
    ///
    /// `#[cfg(test)]`: no production caller needs the list on its own — every
    /// production reader either walks `self.xrefs` directly (`resolve`) or
    /// goes through `render`'s combination of it with `template`.
    #[cfg(test)]
    pub(crate) fn xrefs(&self) -> &[XrefSegment] {
        &self.xrefs
    }

    /// Renders the footnote text from the template and the current (resolved or
    /// unresolved) state of its cross-references.
    ///
    /// The template is the builder's fold of the footnote's subtree, which
    /// never enters escaped sentinel form — see [`render_xref_template`].
    pub(crate) fn render(&self, renderer: &dyn InlineRenderer) -> String {
        render_xref_template(&self.template, &self.xrefs, renderer)
    }

    /// Resolves the footnote's cross-references using `resolver`, reporting any
    /// unresolved target in `warnings` against `source`. Rendering the resolved
    /// text is left to the caller (via [`render`](Self::render)).
    ///
    /// Every cross-reference in [`xrefs`](Self::xrefs) is resolved and
    /// warned about uniformly here, whether or not `template` can address
    /// it by index — see the type's own docs.
    pub(crate) fn resolve<'src>(
        &mut self,
        resolver: &dyn ReferenceResolver,
        warnings: &mut ReferenceWarnings<'src>,
        source: Span<'src>,
    ) {
        for xref in self.xrefs.iter_mut() {
            // The catalog holds the document's own text, which is exactly
            // what a tree-read segment's target carries.
            let resolved = resolver.resolve(&ResolutionContext {
                target: &xref.target,
                provided_text: xref.provided_text.as_deref(),
                derived: xref.derived.as_ref(),
            });

            if resolved.is_none() && xref.derived.is_none() {
                warnings.unresolved(&xref.target, source);
            }

            xref.resolved = resolved;
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
            && self.deferred == other.deferred
            && self.passthroughs == other.passthroughs
    }
}

impl Eq for Content<'_> {}

impl std::hash::Hash for Content<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.original.hash(state);
        self.rendered.hash(state);
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

/// Collects every **defining** footnote occurrence in `nodes`, in document
/// order.
///
/// A defining occurrence carries the footnote's children; a bare reference to
/// an existing footnote carries none and re-uses the defining entry. Both
/// [`Content::collect_folded_footnotes`] — which folds these — and the
/// retention of a content's render attributes at parse time ask the same
/// question of the same tree, so they ask it through one function.
fn defining_footnotes<'a, 'src>(
    nodes: &'a [InlineNode<'src>],
    out: &mut Vec<(&'a str, &'a [InlineNode<'src>])>,
) {
    for node in nodes {
        match node {
            InlineNode::Footnote(footnote) => {
                // The number is part of what makes an occurrence *defining*:
                // `Parser::define_footnote` assigns one to every footnote it
                // registers, and only a reference that resolved to nothing is
                // left without. Asking both questions in one condition is
                // deliberate — asked separately, the second would have an arm
                // no input can reach.
                if !footnote.is_reference
                    && let Some(number) = footnote.number.as_ref()
                {
                    out.push((number.as_ref(), &footnote.children));
                }

                defining_footnotes(&footnote.children, out);
            }
            InlineNode::Styled(styled) => defining_footnotes(&styled.children, out),
            InlineNode::Ref(reference) => defining_footnotes(&reference.children, out),
            InlineNode::IndexTerm(term) => defining_footnotes(&term.children, out),
            InlineNode::Stem(stem) => defining_footnotes(&stem.children, out),
            _ => {}
        }
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
            // A filtered view byte-identical to the source span is borrowed
            // from the span rather than allocating an owned copy of
            // text we already hold.
            let content = Content::from_filtered(Span::new("plain text"), "plain text");

            assert!(matches!(content.rendered, CowStr::Borrowed(_)));
            assert_eq!(content.rendered.as_ref(), "plain text");
        }

        #[test]
        fn owns_rendered_text_when_filter_changes_it() {
            // A filtered view that differs from the source is materialized as
            // an owned copy.
            let content = Content::from_filtered(Span::new("a|b"), "ab");

            assert!(matches!(content.rendered, CowStr::Boxed(_)));
            assert_eq!(content.rendered.as_ref(), "ab");
        }
    }

    mod impl_debug {
        use crate::{Span, content::Content};

        #[test]
        fn shows_the_deferred_state_only_when_there_is_one() {
            let mut content = Content::from(Span::new("see <<a>>"));

            // The cross-reference-free case (the overwhelming majority) debugs
            // as a plain `original` + `rendered` pair.
            assert!(!format!("{content:?}").contains("deferred"));

            // Installing the deferred state the way production does: the
            // group apply derives it from the tree (`set_tree_xrefs`).
            crate::content::SubstitutionGroup::Normal.apply(
                &mut content,
                &crate::Parser::default(),
                None,
            );

            let debug = format!("{content:?}");
            assert!(debug.contains("deferred"), "{debug:?}");
            assert!(debug.contains("XrefSegment"), "{debug:?}");
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

    mod render_xref_template {
        use super::super::{XrefSegment, XrefTemplatePiece, render_xref_template};
        use crate::parser::HtmlInlineRenderer;

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
        fn splices_a_reference_between_literal_pieces() {
            let template = [
                XrefTemplatePiece::Literal("see ".to_string()),
                XrefTemplatePiece::Xref(0),
                XrefTemplatePiece::Literal(" here".to_string()),
            ];

            assert_eq!(
                render_xref_template(&template, &[segment("a")], &HtmlInlineRenderer {}),
                r##"see <a href="#a">[a]</a> here"##
            );
        }

        #[test]
        fn a_literal_needs_no_escaping_to_stay_literal() {
            // The point of the structured template: a literal that happens to
            // hold the old in-band placeholder byte sequence is just bytes,
            // spliced around rather than read back — so the escaping the
            // string form needed has nothing left to protect here.
            let template = [
                XrefTemplatePiece::Literal("x\u{e000}0\u{e001}y ".to_string()),
                XrefTemplatePiece::Xref(0),
            ];

            assert_eq!(
                render_xref_template(&template, &[segment("a")], &HtmlInlineRenderer {}),
                "x\u{e000}0\u{e001}y <a href=\"#a\">[a]</a>"
            );
        }

        #[test]
        fn an_xref_piece_past_the_segment_list_renders_nothing() {
            // Unreachable from `carried_title_template`, which assigns each
            // index from its position in the very list it travels with; pinned
            // so the guard is a defined behavior rather than a dead branch.
            let template = [
                XrefTemplatePiece::Literal("a".to_string()),
                XrefTemplatePiece::Xref(9),
                XrefTemplatePiece::Literal("b".to_string()),
            ];

            assert_eq!(
                render_xref_template(&template, &[segment("a")], &HtmlInlineRenderer {}),
                "ab"
            );
        }
    }

    mod footnote_deferred {
        use super::super::{FootnoteDeferred, XrefSegment, XrefTemplatePiece};

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
        fn debug_includes_template_and_xrefs() {
            let deferred = FootnoteDeferred::new(
                vec![XrefTemplatePiece::Literal("t".to_string())],
                vec![segment("a")],
            );
            let rendered = format!("{deferred:?}");
            assert!(rendered.contains("FootnoteDeferred"));
            assert!(rendered.contains("template"));
        }
    }
}
