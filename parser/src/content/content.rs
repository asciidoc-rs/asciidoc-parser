//! Describes the content of a non-compound block after any relevant
//! [substitutions] have been performed.
//!
//! [substitutions]: https://docs.asciidoctor.org/asciidoc/latest/subs/

use std::borrow::Cow;

use crate::{
    Span,
    content::Passthrough,
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
#[derive(Clone, Eq, Hash, PartialEq)]
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
}

/// The deferred (cross-reference-bearing) portion of a [`Content`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

/// Sentinel codepoints (Unicode Private Use Area) bracketing a footnote's
/// rendered inline marker while a section title is being substituted. Like the
/// cross-reference placeholders above, they are inert to the remaining
/// substitution steps, and a document's own copies are escaped out of the way
/// first (see [`escape_sentinels`]).
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
const RESERVED_SENTINELS: [(char, char); 7] = [
    (XREF_PLACEHOLDER_START, 'a'),
    (XREF_PLACEHOLDER_END, 'b'),
    (FOOTNOTE_MARKER_START, 'c'),
    (FOOTNOTE_MARKER_END, 'd'),
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
/// [`rehome_xref_placeholders`], [`strip_footnote_marker_spans`], and the
/// passthrough restore — cannot otherwise tell a sentinel the parser wrote
/// from one the document did. Without this pass, a document that types
/// `U+E000 0 U+E001` alongside a real cross-reference has that text read back
/// as a placeholder, forging a second cross-reference into the output.
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

/// Returns `true` if `c` is one of the codepoints the substitution pipeline
/// reserves for its own use.
///
/// Written as a range test rather than a scan of [`RESERVED_SENTINELS`]: this
/// runs over every character of every block's content, while the table itself
/// is only consulted for text that has a sentinel in it. A unit test pins the
/// two to the same set of codepoints.
fn is_reserved_sentinel(c: char) -> bool {
    matches!(c, '\u{96}' | '\u{97}' | '\u{E000}'..='\u{E004}')
}

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
        // without an end cannot occur — the substitution always emits both —
        // but if it somehow did, drop the remainder rather than
        // reintroduce the stray sentinel.
        rest = match rest.find(FOOTNOTE_MARKER_END) {
            Some(end) => &rest[end + FOOTNOTE_MARKER_END.len_utf8()..],
            None => "",
        };
    }

    out.push_str(rest);
    out
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

    /// The placeholder template and cross-references, when the title carries
    /// any; `None` for the (overwhelmingly common) cross-reference-free title.
    deferred: Option<(String, Vec<XrefSegment>)>,
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
            passthroughs: Vec::new(),
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
        // `debug_assert_eq!` message reports both counts if this is ever
        // broken.
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

    /// Returns the inline passthroughs extracted from this content during
    /// substitution, in extraction order.
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
    pub fn passthroughs(&self) -> &[Passthrough] {
        &self.passthroughs
    }

    /// Records the inline passthroughs extracted from this content during
    /// substitution, so they remain observable via
    /// [`passthroughs`](Self::passthroughs) after the restore pass has spliced
    /// them back in.
    pub(crate) fn set_passthroughs(&mut self, passthroughs: Vec<Passthrough>) {
        self.passthroughs = passthroughs;
    }

    /// Escapes the reserved sentinel codepoints this content's *own text*
    /// contains, so the substitution pipeline can tell its own in-band
    /// sentinels from the document's text. See [`escape_sentinels`].
    ///
    /// Called once, before substitution begins; the matching
    /// [`unescape_sentinels`](Self::unescape_sentinels) call restores them.
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
    pub(crate) fn unescape_sentinels(&mut self) {
        if let Cow::Owned(unescaped) = unescape_sentinels(self.rendered.as_ref()) {
            self.rendered = unescaped.into();
        }
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
                // A target is matched against the catalog, which holds the
                // document's own text (an ID as it was written, a section's
                // reference text). The segment keeps the target in escaped form
                // — it is rendered back into escaped text — so the key handed
                // to the resolver leaves escaped form here. See
                // [`escape_sentinels`].
                xref.resolved = resolver.resolve(&ResolutionContext {
                    target: &unescape_sentinels(&xref.target),
                    provided_text: xref.provided_text.as_deref(),
                    derived: xref.derived.as_ref(),
                });

                // A reference whose placeholder is no longer in the template
                // was re-homed into a footnote (see
                // `rehome_xref_placeholders`); the
                // footnote resolves and reports it, so it is not reported here.
                // A target that names a document is never reported: it
                // carries its own destination, so there was nothing here to
                // resolve.
                if xref.resolved.is_none()
                    && xref.derived.is_none()
                    && template.contains(&Content::xref_placeholder(index))
                {
                    warnings.unresolved(&unescape_sentinels(&xref.target), source);
                }
            }
        }

        // Content that carries no deferred cross-reference is untouched here:
        // its rendering left escaped form at the end of substitution, and
        // unescaping again would read a document's own escape sequence as one
        // of ours.
        if self.deferred.is_some() {
            self.rebuild_rendered(renderer);

            // The template is held in escaped form, so the rebuilt rendering is
            // too; hand it back as the document wrote it.
            self.unescape_sentinels();
        }
    }

    /// Rebuilds [`Content::rendered`] from the deferred template and the
    /// current (resolved or unresolved) state of its cross-references.
    fn rebuild_rendered(&mut self, renderer: &dyn InlineSubstitutionRenderer) {
        // Both callers establish that there is a template to rebuild from, so
        // the guard here is only a safety net.
        if let Some(deferred) = self.deferred.as_ref() {
            self.rendered = render_template(&deferred.template, &deferred.xrefs, renderer).into();
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
    // The template is held in escaped form (see `escape_sentinels`); this is
    // the title's final rendering, so it leaves escaped form here.
    unescape_sentinels(&render_template(template, xrefs, renderer)).into_owned()
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
                // Not a placeholder this template owns: emit the text
                // literally. Placeholder indices are assigned sequentially into
                // the same `Content`'s `xrefs`, so this is unreachable for a
                // placeholder the substitution wrote; text is never rejected
                // here (nor asserted against) because a sequence that merely
                // looks like a placeholder is content, and content is passed
                // through.
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
        // The template is held in escaped form (see `escape_sentinels`); the
        // footnote's text is user-facing, so it leaves escaped form here.
        unescape_sentinels(&render_template(&self.template, &self.xrefs, renderer)).into_owned()
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
            // leaves escaped form here (see `Content::resolve_references`).
            xref.resolved = resolver.resolve(&ResolutionContext {
                target: &unescape_sentinels(&xref.target),
                provided_text: xref.provided_text.as_deref(),
                derived: xref.derived.as_ref(),
            });

            if xref.resolved.is_none() && xref.derived.is_none() {
                warnings.unresolved(&unescape_sentinels(&xref.target), source);
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
            FOOTNOTE_MARKER_END, FOOTNOTE_MARKER_START, PASSTHROUGH_PLACEHOLDER_END,
            PASSTHROUGH_PLACEHOLDER_START, RESERVED_SENTINELS, SENTINEL_ESCAPE,
            XREF_PLACEHOLDER_END, XREF_PLACEHOLDER_START, escape_sentinels, is_reserved_sentinel,
            unescape_sentinels,
        };

        #[test]
        fn the_range_test_and_the_table_describe_the_same_codepoints() {
            for (sentinel, _) in RESERVED_SENTINELS {
                assert!(
                    is_reserved_sentinel(sentinel),
                    "{sentinel:?} is reserved but not covered by the range test"
                );
            }

            for c in '\u{0}'..=char::MAX {
                assert_eq!(
                    is_reserved_sentinel(c),
                    RESERVED_SENTINELS
                        .iter()
                        .any(|(sentinel, _)| *sentinel == c),
                    "the range test and the table disagree about {c:?}"
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
                "a{XREF_PLACEHOLDER_START}0{XREF_PLACEHOLDER_END}b{FOOTNOTE_MARKER_START}\
                 c{FOOTNOTE_MARKER_END}d{PASSTHROUGH_PLACEHOLDER_START}1\
                 {PASSTHROUGH_PLACEHOLDER_END}e{SENTINEL_ESCAPE}f"
            );

            let escaped = escape_sentinels(&typed);

            for reserved in [
                XREF_PLACEHOLDER_START,
                XREF_PLACEHOLDER_END,
                FOOTNOTE_MARKER_START,
                FOOTNOTE_MARKER_END,
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
            assert_eq!(escaped.matches(SENTINEL_ESCAPE).count(), 7, "{escaped:?}");

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
            // so this only pins down that a malformed sequence is content, not
            // a dropped character.
            let dangling = format!("x{SENTINEL_ESCAPE}");
            assert_eq!(unescape_sentinels(&dangling), dangling);

            let unknown_tag = format!("x{SENTINEL_ESCAPE}zy");
            assert_eq!(unescape_sentinels(&unknown_tag), unknown_tag);
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
            // Defensive: the substitution always emits balanced sentinels, but
            // a lone start must not leak the sentinel into the
            // output.
            let input = format!("Title{FOOTNOTE_MARKER_START}dangling");
            assert_eq!(strip_footnote_marker_spans(&input), "Title");
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
            XREF_PLACEHOLDER_END, XREF_PLACEHOLDER_START, XrefSegment, render_template,
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
            render_template(template, xrefs, &HtmlSubstitutionRenderer {})
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
            // as-is (these cannot arise in practice, but the fallback is
            // exercised).
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
