//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use std::borrow::Cow;

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

    /// The placeholder template, in escaped sentinel form: every
    /// document-derived byte escaped, every raw `U+E000 <index> U+E001` a real
    /// placeholder.
    ///
    /// Two producers write one, and only one of them reaches a production
    /// reader. The string pipeline still captures its own through
    /// [`Content::finalize_deferred`], as it always did — that copy is what the
    /// test-only oracle path renders from, and it goes with `run_pipeline`.
    /// The one content that *renders* from a template in production — a block
    /// title carried across a section heading, which travels through
    /// `Parser::pending_block_title` as an [`OwnedTitle`] because the parser it
    /// rides on has no `'src` lifetime, and so arrives at the claiming block
    /// with its inline nodes dropped — carries a template synthesized **from
    /// its own tree** at the hop instead (see [`Content::to_owned_title`] and
    /// [`carried_title_template`]). Its rendering cannot be a fold, so it stays
    /// a splice — see [`Content::rebuild_rendered`] — but the splice's inputs
    /// no longer come from the string pipeline.
    ///
    /// Empty between [`Content::set_deferred_xrefs`] recording the list and
    /// `finalize_deferred` capturing the template, which is within one
    /// `run_pipeline` call and reaches no reader.
    template: String,

    /// Whether [`block`](Self::block) and [`footnote`](Self::footnote) were
    /// read off this content's **inline tree**, rather than being the string
    /// pipeline's own flat list.
    ///
    /// Always `true` for a production content: [`Content::set_tree_xrefs`] is
    /// the only producer of deferred state at the seam now that the string
    /// pipeline no longer runs there. `false` arises only through
    /// [`Content::set_deferred_xrefs`], on the test-only oracle path
    /// (`apply_string_pipeline`), and the `!from_tree` branches this field
    /// still gates — the escaped-form reads, the template partition — are that
    /// path's and go with `run_pipeline`.
    from_tree: bool,
}

impl DeferredContent {
    /// Which halves of this content's template inputs are in escaped sentinel
    /// form.
    ///
    /// The template is always the string pipeline's — it is the only pass that
    /// writes one — while the segments are the tree's wherever the carve-out
    /// did not fire, and the tree's are the document's own text already.
    fn escaped_form(&self) -> EscapedForm {
        EscapedForm {
            template: true,
            segments: !self.from_tree,
        }
    }
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
/// index in [`DeferredContent::template`].
///
/// A document can type these codepoints — they are unassigned by Unicode, but
/// perfectly valid in a `&str` — so they are *not* self-evidently the parser's
/// own. What keeps them unambiguous is the escaping pass described on
/// [`escape_sentinels`]: a document's own copies are replaced before
/// substitution begins, so every occurrence the readers below see was written
/// by the substitution pipeline itself.
const XREF_PLACEHOLDER_START: char = '\u{E000}';
const XREF_PLACEHOLDER_END: char = '\u{E001}';

/// Sentinel codepoints (C1 control range) bracketing an extracted
/// passthrough's index while the other substitutions run. Defined here so the
/// escaping pass below covers them alongside the Private Use Area sentinels;
/// they are emitted and consumed by
/// [`passthroughs`](crate::content::passthroughs).
pub(crate) const PASSTHROUGH_PLACEHOLDER_START: char = '\u{96}';
pub(crate) const PASSTHROUGH_PLACEHOLDER_END: char = '\u{97}';

/// Introduces the escaped form of a reserved sentinel (see
/// [`escape_sentinels`]). A document's own copies of this codepoint are
/// escaped too, so an occurrence in escaped text always introduces an escape.
const SENTINEL_ESCAPE: char = '\u{E004}';

/// Every codepoint the substitution pipeline reserves as an in-band control
/// sentinel, paired with the ASCII tag that stands in for it in escaped text.
///
/// The tags are arbitrary; all that matters is that each is distinct and that
/// the escape introducer itself is in the table, so escaping is reversible.
const RESERVED_SENTINELS: [(char, char); 5] = [
    (XREF_PLACEHOLDER_START, 'a'),
    (XREF_PLACEHOLDER_END, 'b'),
    (PASSTHROUGH_PLACEHOLDER_START, 'e'),
    (PASSTHROUGH_PLACEHOLDER_END, 'f'),
    (SENTINEL_ESCAPE, 'g'),
];

/// Replaces each reserved sentinel codepoint a *document* typed with an escaped
/// form, so that the only unescaped sentinels in the text being substituted are
/// the ones the substitution pipeline wrote itself.
///
/// The sentinels are in-band: they are spliced into the same string as the
/// document's text, so the passes that read them back — [`render_template`],
/// [`rehome_xref_placeholders`], and the passthrough restore — cannot otherwise
/// tell a sentinel the parser wrote from one the document did. Without this
/// pass, a document that types `U+E000 0 U+E001` alongside a real
/// cross-reference has that text read back as a placeholder, forging a second
/// cross-reference into the output.
///
/// This is the **string pipeline's** own protection, and it needs no
/// counterpart in the single-pass builder: the builder recognizes constructs by
/// range over the source rather than by scanning a rendered string for its own
/// marks, so a codepoint the document typed is never read as one of the
/// parser's. What escaped form still reaches past `run_pipeline` is marked as
/// such — see [`document_text`].
///
/// Escaped text is an internal representation: every path that hands rendered
/// text back to a caller reverses it with [`unescape_sentinels`], so a
/// document's private-use characters survive to the output unchanged. The two
/// passes are exact inverses, so applying them in matched pairs nests safely
/// (a passthrough's text, for example, is substituted by a nested
/// escape/unescape pair while it is itself held in escaped form).
///
/// Text with no reserved codepoint — the overwhelming majority — is borrowed
/// through unchanged.
pub(crate) fn escape_sentinels(text: &str) -> Cow<'_, str> {
    if !text.contains(is_reserved_sentinel) {
        return Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());

    for c in text.chars() {
        match RESERVED_SENTINELS
            .iter()
            .find(|(sentinel, _)| *sentinel == c)
        {
            Some((_, tag)) => {
                out.push(SENTINEL_ESCAPE);
                out.push(*tag);
            }
            None => out.push(c),
        }
    }

    Cow::Owned(out)
}

/// Restores the document's own sentinel codepoints, reversing
/// [`escape_sentinels`].
///
/// Applied once to each string as it leaves the substitution machinery. A
/// dangling escape introducer cannot occur in text this pass is given (escaping
/// always emits a tag), but is passed through literally rather than dropped if
/// it somehow does.
pub(crate) fn unescape_sentinels(text: &str) -> Cow<'_, str> {
    if !text.contains(SENTINEL_ESCAPE) {
        return Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c != SENTINEL_ESCAPE {
            out.push(c);
            continue;
        }

        match chars
            .peek()
            .and_then(|tag| RESERVED_SENTINELS.iter().find(|(_, t)| t == tag))
        {
            Some((sentinel, _)) => {
                out.push(*sentinel);
                chars.next();
            }
            None => out.push(SENTINEL_ESCAPE),
        }
    }

    Cow::Owned(out)
}

/// `text` as the document wrote it: reverses [`escape_sentinels`] when `text`
/// is held in that form, and borrows it through untouched when it is not.
///
/// Which it is cannot be recovered from the text, and guessing is the whole
/// hazard — unescaping something that was never escaped corrupts a value that
/// legitimately contains the escape introducer, which is precisely the
/// confusion the escaping exists to end. So every caller says.
///
/// The three answers in this module: a segment the **string pipeline** deferred
/// is escaped and one read off the **tree** is not
/// ([`DeferredContent::from_tree`]); a placeholder template's own literal text
/// is escaped for a `DeferredContent` and follows its producer for a
/// [`FootnoteDeferred`]; and a **resolved** or **derived** destination is never
/// escaped, the resolver having been handed the document's own text and
/// answering in kind. That last one is why the decoding happens here, piece by
/// piece, rather than over a completed rendering: a rendering splices the
/// resolver's answer into the template, and a pass over the result would decode
/// bytes the resolver supplied.
pub(crate) fn document_text(text: &str, escaped: bool) -> Cow<'_, str> {
    if escaped {
        unescape_sentinels(text)
    } else {
        Cow::Borrowed(text)
    }
}

/// Which halves of a placeholder template's inputs are held in the string
/// pipeline's escaped sentinel form (see [`escape_sentinels`]).
///
/// The two are independent: a `DeferredContent` always carries the string
/// pipeline's own template — it is the only pass that writes one — while its
/// *segments* are the tree's wherever the carve-out did not fire. A
/// [`FootnoteDeferred`] has one producer for both halves.
#[derive(Clone, Copy, Debug)]
struct EscapedForm {
    /// The template's own literal text — everything between the placeholders.
    template: bool,

    /// The segments' [`target`](XrefSegment::target),
    /// [`provided_text`](XrefSegment::provided_text),
    /// [`window`](XrefSegment::window) and [`roles`](XrefSegment::roles): the
    /// fields read back out of the substituted text.
    ///
    /// Never [`resolved`](XrefSegment::resolved) or
    /// [`derived`](XrefSegment::derived), which are the resolver's own answer
    /// and are spliced verbatim.
    segments: bool,
}

/// Returns `true` if `c` is one of the codepoints the substitution pipeline
/// reserves for its own use.
///
/// Written as an inline match rather than a scan of [`RESERVED_SENTINELS`]:
/// this runs over every character of every block's content, while the table
/// itself is only consulted for text that has a sentinel in it. A unit test
/// pins the two to the same set of codepoints.
///
/// `\u{E002}` and `\u{E003}` are deliberately absent: they were the
/// footnote-marker sentinels, and this branch's section-title path derives a
/// heading's reference text by folding its inline subtree instead, so nothing
/// reserves them any more (design §4.2's first sentinel system).
fn is_reserved_sentinel(c: char) -> bool {
    matches!(
        c,
        '\u{96}' | '\u{97}' | '\u{E000}' | '\u{E001}' | '\u{E004}'
    )
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
    ///
    /// A title that defers a cross-reference and still holds its tree swaps its
    /// placeholder template — and the segment list the template's indices point
    /// into — for the **tree's own**, synthesized here by
    /// [`carried_title_template`]. The snapshot travels across the
    /// `'src`-erasing hop (`Parser::pending_block_title`) with its inline nodes
    /// dropped, so the claiming block's title is the one content that renders
    /// from a template rather than folding a tree; this is where that template
    /// stops being the string pipeline's and becomes a product of the tree,
    /// which is what lets the oracle pipeline be deleted without losing the
    /// carried title's resolution.
    ///
    /// `parser` supplies the renderer; the fold runs under the title's own
    /// retained render attributes, the same pairing [`refold`](Self::refold)
    /// uses. A deferring content always retains them
    /// (`set_render_attributes` is called for exactly that content), so the
    /// binding below always takes for a title with a tree; it is written as a
    /// binding rather than an unwrap because the invariant lives in that
    /// pairing, not in the field's type. The condition's other exits are real:
    /// a title **re-stashed** past an empty section body arrives here with its
    /// nodes already dropped and keeps the template the first hop synthesized,
    /// and a `from_tree` `false` snapshot keeps the string pipeline's own — the
    /// only honest one where the tree is known not to describe the content.
    pub(crate) fn to_owned_title(&self, parser: &Parser) -> OwnedTitle {
        let deferred = self.deferred.as_ref().map(|d| {
            let mut d = (**d).clone();

            if d.from_tree
                && !self.inlines.is_empty()
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

    /// Escapes the reserved sentinel codepoints this content's *own text*
    /// contains, so the substitution pipeline can tell its own in-band
    /// sentinels from the document's text. See [`escape_sentinels`].
    ///
    /// Called once, before substitution begins; the matching
    /// [`unescape_sentinels`](Self::unescape_sentinels) call restores them.
    // Vestigial: reachable only from the test-only `run_pipeline` oracle
    // (`apply_string_pipeline`); goes with it.
    #[allow(dead_code)]
    pub(crate) fn escape_sentinels(&mut self) {
        if let Cow::Owned(escaped) = escape_sentinels(self.rendered.as_ref()) {
            self.rendered = escaped.into();
        }
    }

    /// Restores the document's own sentinel codepoints in
    /// [`rendered`](Self::rendered), reversing
    /// [`escape_sentinels`](Self::escape_sentinels).
    ///
    /// The deferred template is deliberately left escaped: it is an internal
    /// representation that is re-rendered (through [`render_template`], whose
    /// callers unescape the result) each time references are resolved.
    // Vestigial: reachable only from the test-only `run_pipeline` oracle
    // (`apply_string_pipeline`); goes with it.
    #[allow(dead_code)]
    pub(crate) fn unescape_sentinels(&mut self) {
        if let Cow::Owned(unescaped) = unescape_sentinels(self.rendered.as_ref()) {
            self.rendered = unescaped.into();
        }
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
    /// Reached only through `run_pipeline`, which no longer runs in production:
    /// what this records is the string pipeline's own answer — the golden every
    /// differential corpus on this branch takes through
    /// [`apply_string_pipeline`](crate::content::SubstitutionGroup) — and it
    /// goes with `run_pipeline` itself. The production list is installed by
    /// [`set_tree_xrefs`](Self::set_tree_xrefs), from the tree, on a content
    /// this was never called on.
    ///
    /// The list lands in [`block`](DeferredContent::block) because the string
    /// pipeline does not partition: a footnote-embedded reference is told apart
    /// by its placeholder having left the template.
    pub(crate) fn set_deferred_xrefs(&mut self, xrefs: Vec<XrefSegment>) {
        if xrefs.is_empty() {
            return;
        }

        debug_assert!(
            self.deferred.is_none(),
            "set_deferred_xrefs must be called at most once per Content"
        );

        self.deferred = Some(Box::new(DeferredContent {
            block: xrefs,
            footnote: Vec::new(),
            template: String::new(),
            from_tree: false,
        }));
    }

    /// Installs this content's deferred cross-references, **read off its own
    /// inline tree** — the sole producer of a production content's deferred
    /// state now that the string pipeline no longer runs at the seam.
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
    /// would traverse every tree twice to build two empty vectors. (The old
    /// early return keyed this on the string pipeline having deferred
    /// something; the walk is the same question asked of the tree.)
    ///
    /// The carve-out that used to live here — keeping the string pipeline's
    /// whole answer where the tree held fewer cross-references than the
    /// pipeline deferred — is gone with the pipeline it fell back to. It was
    /// measured unreachable before the deletion (zero hits across the suite,
    /// both of its member forms closed by their own increments); a form the
    /// builder still declines is simply not a cross-reference of this content
    /// any more, which is the documented-divergence reading every such form
    /// already has.
    pub(crate) fn set_tree_xrefs(
        &mut self,
        tree: &[InlineNode<'src>],
        renderer: &dyn InlineSubstitutionRenderer,
        context: &crate::parser::RenderContext,
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
            template: String::new(),
            from_tree: true,
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
    // Vestigial: reachable only from the test-only `run_pipeline` oracle
    // (`apply_string_pipeline`); goes with it.
    #[allow(dead_code)]
    pub(crate) fn finalize_deferred(&mut self, renderer: &dyn InlineSubstitutionRenderer) {
        let template = self.rendered.as_ref().to_string();

        {
            let Some(deferred) = self.deferred.as_mut() else {
                return;
            };

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
    // Vestigial: reachable only from the test-only `run_pipeline` oracle
    // (`apply_string_pipeline`); goes with it.
    #[allow(dead_code)]
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

            // A string-pipeline content always reaches here with its template
            // captured: `finalize_deferred` sets it at the end of the same
            // `run_pipeline` call that recorded the list. An empty one there
            // means that call was skipped (a future-refactor hazard); the
            // `template.contains` guard below would then silently suppress
            // every unresolved-ref warning, so catch that invariant break in
            // debug builds. A tree content carries no template and never
            // consults one — `reports_unresolved` short-circuits on
            // `from_tree` — so the assert does not apply to it.
            debug_assert!(from_tree || !template.is_empty());

            for (index, xref) in deferred.block.iter_mut().enumerate() {
                // The catalog holds the document's own text (an ID as it was
                // written, a section's reference text), so the key handed to
                // the resolver leaves the string pipeline's escaped sentinel
                // form here. See `document_text`.
                let target = document_text(&xref.target, !from_tree);

                let resolved = resolver.resolve(&ResolutionContext {
                    target: &target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });

                let reports_unresolved =
                    from_tree || template.contains(&Content::xref_placeholder(index));

                // A target that names a document is never reported: it carries
                // its own destination, so there was nothing here to resolve.
                if resolved.is_none() && xref.derived.is_none() && reports_unresolved {
                    warnings.unresolved(&target, source);
                }

                xref.resolved = resolved;
            }

            for xref in deferred.footnote.iter_mut() {
                // The string pipeline produces one flat list, all of it
                // block-level (`set_deferred_xrefs`), so this list is only ever
                // the tree's — but it is read through `document_text` all the
                // same, so the two loops say the same thing about their keys.
                let target = document_text(&xref.target, !from_tree);

                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &target,
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });
            }
        }

        let from_tree = self.resolve_tree_references();

        // Independent of the arm chosen below. A content whose *only*
        // cross-references sit inside its footnotes defers nothing itself — the
        // replacer captures those onto the footnote's own state — so it reaches
        // here with no deferred cross-references and takes the template arm,
        // while its footnotes are exactly the ones needing a fresh rendering.
        // Gating the fold on this content's own deferral would therefore miss
        // the footnotes that most need it.
        self.collect_own_folded_footnotes(renderer, parser, warnings);

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
            // `rebuild_rendered` emits the document's own text directly — the
            // template's literal runs leave escaped form as they are spliced,
            // so the resolver's own answer is never decoded along with them.
            // See `render_template`.
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
    /// The caller gates this on the content **holding a tree**, which every
    /// production content with deferred state does — the deferred lists are
    /// read off that tree, the only producer left at the seam. What takes the
    /// other arm is a content with no tree at all: the carried block title,
    /// which renders from its synthesized template instead (see
    /// [`carried_title_template`]).
    ///
    /// This runs **instead of**
    /// [`rebuild_rendered`](Self::rebuild_rendered), not after it: rendering
    /// both and discarding one would be observable rather than merely
    /// wasteful, since a stateful host renderer would see every callback for
    /// this content twice in a single resolution pass.
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
        renderer: &dyn InlineSubstitutionRenderer,
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
        renderer: &dyn InlineSubstitutionRenderer,
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

        self.rendered = render_template(
            &deferred.template,
            &deferred.block,
            renderer,
            deferred.escaped_form(),
        )
        .into();
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
pub(crate) fn xref_segment_from_node(
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

/// Synthesizes the placeholder template — and the segment list its indices
/// point into — that a **carried block title** takes across the `'src`-erasing
/// hop (see [`Content::to_owned_title`]), from the title's own inline tree.
///
/// The construction is a walk over the tree's **top-level** nodes: a
/// cross-reference node contributes a raw `U+E000 <index> U+E001` placeholder
/// and its [`XrefSegment`] (via [`xref_segment_from_node`], the same reading
/// every other segment derivation uses), and every other node contributes its
/// own fold, passed through [`escape_sentinels`]. That escape is what makes the
/// template's two byte populations distinguishable — the property the whole
/// splice rests on, and the reason the template is *not* one
/// [`fold_deferring_xrefs`](crate::content::inline_builder::fold_deferring_xrefs)
/// call: in that fold's output a document-typed `U+E000 0 U+E001` (see the
/// `sentinels` test module, issue #1235) is byte-identical to a real
/// placeholder, and nothing downstream can tell them apart. Here every
/// document-derived byte is escaped and every raw sentinel is the fold's own —
/// exactly the escaped form the string pipeline's templates have always been
/// in, so the render path ([`render_template`], `EscapedForm { template: true,
/// … }`) needs no new case.
///
/// The price of reading only the top level is a cross-reference **nested**
/// inside another top-level construct (a styled span's children, a visible
/// index term's shown text): its enclosing node folds as a gap, so the
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
    renderer: &dyn InlineSubstitutionRenderer,
    context: &crate::parser::RenderContext,
) -> (String, Vec<XrefSegment>) {
    let mut template = String::new();
    let mut segments = Vec::new();

    for node in nodes {
        match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => {
                let index = segments.len();
                segments.push(xref_segment_from_node(reference, renderer, context));
                template.push_str(&Content::xref_placeholder(index));
            }

            other => {
                let folded = crate::content::inline_builder::fold_html(
                    std::slice::from_ref(other),
                    renderer,
                    context,
                );

                template.push_str(&escape_sentinels(&folded));
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
///
/// `segments_escaped` says whether those segments are the string pipeline's own
/// (see [`EscapedForm`]); the template always is.
pub(crate) fn render_xref_template(
    template: &str,
    xrefs: &[XrefSegment],
    renderer: &dyn InlineSubstitutionRenderer,
    segments_escaped: bool,
) -> String {
    render_template(
        template,
        xrefs,
        renderer,
        EscapedForm {
            template: true,
            segments: segments_escaped,
        },
    )
}

/// Splices resolved (or fallback) cross-reference renderings into a placeholder
/// template, producing the final rendered text.
fn render_template(
    template: &str,
    xrefs: &[XrefSegment],
    renderer: &dyn InlineSubstitutionRenderer,
    form: EscapedForm,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    // The template's own literal text leaves escaped form as it is emitted,
    // rather than the finished rendering being decoded in one pass at the end.
    // That distinction is the whole point: the renderer splices the
    // **resolver's** answer — a destination, a reference text — into this
    // string, and the resolver was handed the document's own text and answered
    // in kind. A pass over the result would decode those bytes too, turning a
    // catalog id that happens to hold the escape introducer into some other
    // sentinel entirely. See [`document_text`].
    let push_literal = |out: &mut String, text: &str| {
        out.push_str(&document_text(text, form.template));
    };

    while let Some(start) = rest.find(XREF_PLACEHOLDER_START) {
        push_literal(&mut out, &rest[..start]);
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
                // The four fields the *substitution* read back out of its own
                // text leave escaped form here; `derived` and `resolved` never
                // entered it. Roles are rebuilt only when one actually carries
                // an escape, which for the overwhelming majority is never.
                let unescaped_roles: Option<Vec<String>> = (form.segments
                    && xref.roles.iter().any(|role| role.contains(SENTINEL_ESCAPE)))
                .then(|| {
                    xref.roles
                        .iter()
                        .map(|role| unescape_sentinels(role).into_owned())
                        .collect()
                });

                renderer.render_xref(
                    &XrefRenderParams {
                        target: &document_text(&xref.target, form.segments),
                        provided_text: xref
                            .provided_text
                            .as_deref()
                            .map(|text| document_text(text, form.segments))
                            .as_deref(),
                        window: xref
                            .window
                            .as_deref()
                            .map(|window| document_text(window, form.segments))
                            .as_deref(),
                        roles: unescaped_roles.as_deref().unwrap_or(&xref.roles),
                        xrefstyle: xref.xrefstyle,
                        derived: xref.derived.as_ref(),
                        resolved: xref.resolved.as_ref(),
                    },
                    &mut out,
                );
            }

            None => {
                // Not a placeholder this template owns: emit the text
                // literally. Placeholder indices are assigned sequentially into
                // the same `Content`'s `xrefs`, so this is unreachable for a
                // placeholder the substitution wrote; text is never rejected
                // here (nor asserted against) because a sequence that merely
                // looks like a placeholder is content, and content is passed
                // through.
                out.push(XREF_PLACEHOLDER_START);
                push_literal(&mut out, body);
                out.push(XREF_PLACEHOLDER_END);
            }
        }
    }

    push_literal(&mut out, rest);
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

    /// Whether [`template`](Self::template) and the segments' targets are held
    /// in the string pipeline's escaped sentinel form (see
    /// [`escape_sentinels`]).
    ///
    /// A footnote registered by the string replacer is; one registered from its
    /// own subtree by the single-pass builder is not, the builder having no
    /// in-band sentinels to hide a document's own codepoints from. The
    /// distinction cannot be recovered from the text, so it is carried — see
    /// [`document_text`], which is the same question for a block's segments.
    sentinels_escaped: bool,
}

impl FootnoteDeferred {
    /// Constructs a footnote's deferred cross-reference state from the
    /// placeholder-bearing `template` and its `xrefs` (in placeholder order).
    ///
    /// `sentinels_escaped` says which pipeline produced them — see the field.
    pub(crate) fn new(template: String, xrefs: Vec<XrefSegment>, sentinels_escaped: bool) -> Self {
        Self {
            template,
            xrefs,
            sentinels_escaped,
        }
    }

    /// Overrides [`sentinels_escaped`](Self::sentinels_escaped).
    ///
    /// For the side-effect differential harness alone, which compares the
    /// footnote catalog entry the string pipeline registers against the one the
    /// builder registers. The two encode their templates differently *by
    /// construction* — that is what the flag records — so the encoding is
    /// normalized away before the entries are compared, leaving the footnote
    /// itself (index, id, rendered text, segments) as the subject.
    #[cfg(test)]
    pub(crate) fn set_sentinels_escaped(&mut self, sentinels_escaped: bool) {
        self.sentinels_escaped = sentinels_escaped;
    }

    /// Renders the footnote text from the template and the current (resolved or
    /// unresolved) state of its cross-references.
    pub(crate) fn render(&self, renderer: &dyn InlineSubstitutionRenderer) -> String {
        // One producer wrote both halves, so they are in the same form.
        render_template(
            &self.template,
            &self.xrefs,
            renderer,
            EscapedForm {
                template: self.sentinels_escaped,
                segments: self.sentinels_escaped,
            },
        )
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
            // The catalog holds the document's own text, so the lookup key
            // leaves escaped form here (see `document_text`).
            let target = document_text(&xref.target, self.sentinels_escaped);

            let resolved = resolver.resolve(&ResolutionContext {
                target: &target,
                provided_text: xref.provided_text.as_deref(),
                derived: xref.derived.as_ref(),
            });

            if resolved.is_none() && xref.derived.is_none() {
                warnings.unresolved(&target, source);
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

/// Collects every **defining** footnote occurrence in `nodes`, in document
/// order.
///
/// A defining occurrence carries the footnote's children; a bare reference to
/// an existing footnote carries none and re-uses the defining entry. Both
/// [`Content::collect_folded_footnotes`] — which folds these — and the
/// retention of a content's render attributes at parse time ask the same
/// question of the same tree, so they ask it through one function.
pub(crate) fn defining_footnotes<'a, 'src>(
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

    mod impl_debug {
        use crate::{
            Span,
            content::{Content, XrefSegment},
        };

        #[test]
        fn shows_the_deferred_state_only_when_there_is_one() {
            let mut content = Content::from(Span::new("see <<a>>"));

            // The cross-reference-free case (the overwhelming majority) debugs
            // as a plain `original` + `rendered` pair.
            assert!(!format!("{content:?}").contains("deferred"));

            content.set_deferred_xrefs(vec![XrefSegment {
                target: "a".to_string(),
                provided_text: None,
                window: None,
                roles: vec![],
                xrefstyle: None,
                derived: None,
                resolved: None,
            }]);

            let debug = format!("{content:?}");
            assert!(debug.contains("deferred"), "{debug:?}");
            assert!(debug.contains("XrefSegment"), "{debug:?}");
        }
    }

    mod escape_sentinels {
        use std::borrow::Cow;

        use super::super::{
            PASSTHROUGH_PLACEHOLDER_END, PASSTHROUGH_PLACEHOLDER_START, RESERVED_SENTINELS,
            SENTINEL_ESCAPE, XREF_PLACEHOLDER_END, XREF_PLACEHOLDER_START, escape_sentinels,
            is_reserved_sentinel, unescape_sentinels,
        };

        #[test]
        fn the_range_test_and_the_table_describe_the_same_codepoints() {
            for (sentinel, _) in RESERVED_SENTINELS {
                assert!(
                    is_reserved_sentinel(sentinel),
                    "{sentinel:?} is reserved but not covered by the inline match"
                );
            }

            for c in '\u{0}'..=char::MAX {
                assert_eq!(
                    is_reserved_sentinel(c),
                    RESERVED_SENTINELS
                        .iter()
                        .any(|(sentinel, _)| *sentinel == c),
                    "the inline match and the table disagree about {c:?}"
                );
            }
        }

        #[test]
        fn borrows_text_with_no_reserved_codepoint() {
            assert!(matches!(
                escape_sentinels("plain text"),
                Cow::Borrowed("plain text")
            ));

            assert!(matches!(
                unescape_sentinels("plain text"),
                Cow::Borrowed("plain text")
            ));
        }

        #[test]
        fn escaped_text_holds_no_reserved_codepoint() {
            let typed = format!(
                "a{XREF_PLACEHOLDER_START}0{XREF_PLACEHOLDER_END}b\
                 {PASSTHROUGH_PLACEHOLDER_START}1{PASSTHROUGH_PLACEHOLDER_END}\
                 e{SENTINEL_ESCAPE}f"
            );

            let escaped = escape_sentinels(&typed);

            for reserved in [
                XREF_PLACEHOLDER_START,
                XREF_PLACEHOLDER_END,
                PASSTHROUGH_PLACEHOLDER_START,
                PASSTHROUGH_PLACEHOLDER_END,
            ] {
                assert!(
                    !escaped.contains(reserved),
                    "escaped text still contains {reserved:?}: {escaped:?}"
                );
            }

            // The escape introducer is the one reserved codepoint that remains,
            // and every occurrence of it is one the escaping wrote.
            assert_eq!(escaped.matches(SENTINEL_ESCAPE).count(), 5, "{escaped:?}");

            assert_eq!(unescape_sentinels(&escaped), typed);
        }

        #[test]
        fn round_trips_an_escape_introducer_followed_by_a_tag() {
            // The sequence a document is most likely to be mangled by: its own
            // escape introducer followed by a character that is also an escape
            // tag. Escaping the introducer keeps the pair distinguishable.
            let typed = format!("x{SENTINEL_ESCAPE}ay");

            assert_eq!(unescape_sentinels(&escape_sentinels(&typed)), typed);
        }

        #[test]
        fn passes_a_dangling_escape_introducer_through() {
            // Cannot arise from `escape_sentinels` (which always writes a tag),
            // so this only pins down that a malformed sequence is content, not a
            // dropped character.
            let dangling = format!("x{SENTINEL_ESCAPE}");
            assert_eq!(unescape_sentinels(&dangling), dangling);

            let unknown_tag = format!("x{SENTINEL_ESCAPE}zy");
            assert_eq!(unescape_sentinels(&unknown_tag), unknown_tag);
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

    mod render_template {
        use super::super::{
            EscapedForm, XREF_PLACEHOLDER_END, XREF_PLACEHOLDER_START, XrefSegment, render_template,
        };
        use crate::parser::HtmlSubstitutionRenderer;

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

        fn render(template: &str, xrefs: &[XrefSegment]) -> String {
            render_template(
                template,
                xrefs,
                &HtmlSubstitutionRenderer {},
                EscapedForm {
                    template: true,
                    segments: true,
                },
            )
        }

        #[test]
        fn splices_a_reference_into_its_placeholder() {
            let template = format!("see {XREF_PLACEHOLDER_START}0{XREF_PLACEHOLDER_END} here");

            assert_eq!(
                render(&template, &[segment("a")]),
                r##"see <a href="#a">[a]</a> here"##
            );
        }

        #[test]
        fn passes_an_unterminated_placeholder_through_literally() {
            // A start sentinel with no end cannot arise from the substitution
            // (which always writes both), and a document's own copy is escaped
            // before it gets here, so this only pins down that the text is
            // emitted rather than swallowed.
            let unterminated = format!("a{XREF_PLACEHOLDER_START}0 no end");

            assert_eq!(render(&unterminated, &[segment("a")]), unterminated);
        }

        #[test]
        fn passes_an_unmatched_placeholder_through_literally() {
            // Likewise for a bracketed body that names no reference this
            // template owns: a sequence that merely looks like a placeholder is
            // content, and reaches the output as content.
            let out_of_range = format!("x{XREF_PLACEHOLDER_START}9{XREF_PLACEHOLDER_END}y");
            assert_eq!(render(&out_of_range, &[segment("a")]), out_of_range);

            let non_numeric = format!("x{XREF_PLACEHOLDER_START}xyz{XREF_PLACEHOLDER_END}y");
            assert_eq!(render(&non_numeric, &[segment("a")]), non_numeric);
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
            let deferred = FootnoteDeferred::new("t".to_string(), vec![segment("a")], true);
            let rendered = format!("{deferred:?}");
            assert!(rendered.contains("FootnoteDeferred"));
            assert!(rendered.contains("template"));
        }
    }
}
