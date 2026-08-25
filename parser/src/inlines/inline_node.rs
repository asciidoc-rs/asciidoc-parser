use crate::{
    HasSpan, Span,
    content::SubstitutionGroup,
    inlines::{Anchor, Callout, CharRef, Footnote, Image, IndexTerm, Ref, Stem, Styled, Ui},
    strings::CowStr,
};

/// One inline node in the inline AST. Borrows text from the source where it
/// can, so the common case (a run of ordinary text) does not allocate.
///
/// The first five variants are the ASG inline core: three literal leaves
/// ([`Text`](Self::Text), [`CharRef`](Self::CharRef), [`Raw`](Self::Raw)) and
/// two parents ([`Styled`](Self::Styled), [`Ref`](Self::Ref)). The remaining
/// variants are crate extensions that the ASG does not yet model; each projects
/// down to an ASG-legal node when emitting conformant ASG.
///
/// Every node carries a `location` [`Span`] (directly or on its inner struct),
/// so [`InlineNode`] implements [`HasSpan`] and locates itself exactly the way
/// blocks already do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlineNode<'src> {
    // ─── ASG literal nodes (leaves) ───────────────────────────────────
    /// Logical text between constructs. `value` is the *reader's* text
    /// (attribute references expanded, but **not** HTML-escaped); the HTML fold
    /// escapes `<`, `>`, and `&`. `location` is the source [`Span`] the text
    /// derives from; for a verbatim run `value` borrows the very slice
    /// `location.data()` covers, and the two coincide. ASG: `inlineLiteral`
    /// with `name="text"`.
    Text {
        /// The reader's text, with attribute references expanded but no
        /// HTML-escaping applied.
        value: CowStr<'src>,

        /// The source location this text derives from.
        location: Span<'src>,
    },

    /// A character reference or typographic replacement: a special character
    /// (`<`, `>`, `&`), a character-replacement result (`(C)`, `--`, `...`,
    /// smart quotes, arrows), or a numeric/named entity. `value` carries the
    /// logical character(s); the fold decides the concrete output entity.
    /// `location` is the source that produced it (for example the `(C)` span).
    /// ASG: `inlineLiteral` with `name="charref"`.
    CharRef {
        /// The logical character reference; the renderer chooses its encoding.
        value: CharRef<'src>,

        /// The source location that produced this reference.
        location: Span<'src>,
    },

    /// Content that later substitution steps must not see inside — passthrough
    /// content (`+++…+++`, `pass:[…]`, `++…++`, `$$…$$`), a masked STEM body,
    /// and any literal special character of an attribute expansion that the
    /// effective substitution order left unescaped. This node is the model's
    /// record of the language's "this text is off limits" behavior. ASG:
    /// `inlineLiteral` with `name="raw"`.
    ///
    /// [`form`](RawForm) says whether `value` is already output
    /// bytes or logical text the fold escapes. Both are opaque to the
    /// transducer steps — that is what makes them one node kind rather than
    /// two — but only one is "raw HTML by design", and conflating them made
    /// a passthrough's escaping a function of whichever renderer the
    /// *parse* carried rather than of the one the fold is given.
    Raw {
        /// The content this node contributes, in the shape [`form`](Self::Raw)
        /// names.
        value: CowStr<'src>,

        /// Whether the fold emits [`value`](Self::Raw) as-is or escapes it.
        form: RawForm,

        /// Where this raw output came from — see [`RawOrigin`].
        origin: RawOrigin,

        /// The source location this content derives from.
        location: Span<'src>,
    },

    // ─── ASG parent nodes ─────────────────────────────────────────────
    /// A formatted span (strong, emphasis, code, mark, and crate extensions).
    /// ASG: `inlineSpan`.
    Styled(Styled<'src>),

    /// A link or cross-reference. ASG: `inlineRef`.
    Ref(Ref<'src>),

    // ─── crate extensions (no ASG inline node yet) ────────────────────
    /// An inline image (`image:target[…]`).
    Image(Image<'src>),

    /// A footnote (`footnote:[…]` or `footnote:id[…]`).
    Footnote(Footnote<'src>),

    /// An inline anchor (`[[id]]` or `anchor:id[reftext]`).
    Anchor(Anchor<'src>),

    /// A UI macro: `kbd:`, `btn:`, or `menu:`.
    Ui(Ui<'src>),

    /// An index term (`((term))`, `(((primary, secondary)))`,
    /// `indexterm:[…]`, or `indexterm2:[…]`).
    IndexTerm(IndexTerm<'src>),

    /// A callout number in verbatim content (`<1>`, `<.>`).
    Callout(Callout<'src>),

    /// Inline STEM content (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`).
    Stem(Stem<'src>),

    /// An explicit line break (a trailing `+` at the end of a line).
    LineBreak {
        /// The source location of the line break.
        location: Span<'src>,
    },
}

impl<'src> HasSpan<'src> for InlineNode<'src> {
    fn span(&self) -> Span<'src> {
        match self {
            Self::Text { location, .. } => *location,
            Self::CharRef { location, .. } => *location,
            Self::Raw { location, .. } => *location,
            Self::Styled(styled) => styled.span(),
            Self::Ref(ref_) => ref_.span(),
            Self::Image(image) => image.span(),
            Self::Footnote(footnote) => footnote.span(),
            Self::Anchor(anchor) => anchor.span(),
            Self::Ui(ui) => ui.span(),
            Self::IndexTerm(index_term) => index_term.span(),
            Self::Callout(callout) => callout.span(),
            Self::Stem(stem) => stem.span(),
            Self::LineBreak { location } => *location,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        HasSpan, Span,
        inlines::{
            Anchor, Callout, CalloutGuard, CharRef, Footnote, Image, IndexTerm, InlineNode,
            RawForm, RawOrigin, Ref, RefVariant, SpanForm, Stem, StemNotation, StyleVariant,
            Styled, Ui, UiKind,
        },
        strings::CowStr,
    };

    // One representative node of every `InlineNode` variant, sharing a single
    // location so every arm of the `HasSpan` match is exercised and can be
    // checked against the same span.
    fn one_of_every_variant(location: Span<'static>) -> Vec<InlineNode<'static>> {
        vec![
            InlineNode::Text {
                value: CowStr::from("hi"),
                location,
            },
            InlineNode::CharRef {
                value: CharRef::Special('<'),
                location,
            },
            InlineNode::Raw {
                value: CowStr::from("<b>"),
                form: RawForm::AsIs,
                origin: RawOrigin::Substitution,
                location,
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location,
            }),
            InlineNode::Ref(Ref {
                variant: RefVariant::Link,
                link_form: Some(crate::inlines::LinkForm::Macro),
                target: CowStr::from("https://example.com"),
                children: vec![],
                roles: vec![],
                window: None,
                resolved: None,
                derived: None,
                xrefstyle: None,
                attrs: None,
                location,
            }),
            InlineNode::Image(Image {
                is_icon: false,
                target: CowStr::from("photo.png"),
                restored_target_ranges: vec![],
                alt: None,
                width: None,
                height: None,
                attrs: None,
                location,
            }),
            InlineNode::Footnote(Footnote {
                id: None,
                number: Some(CowStr::from("1")),
                is_reference: false,
                children: vec![],
                location,
            }),
            InlineNode::Anchor(Anchor {
                id: CowStr::from("intro"),
                reftext: None,
                is_bibliography: false,
                location,
            }),
            InlineNode::Ui(Ui {
                kind: UiKind::Button(CowStr::from("Save")),
                location,
            }),
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![CowStr::from("term")],
                children: vec![],
                visible: false,
                location,
            }),
            InlineNode::Callout(Callout {
                number: CowStr::from("1"),
                guard: CalloutGuard::LineComment(CowStr::from("# ")),
                location,
            }),
            InlineNode::Stem(Stem {
                notation: StemNotation::AsciiMath,
                value: CowStr::from("x^2"),
                location,
            }),
            InlineNode::LineBreak { location },
        ]
    }

    #[test]
    fn locates_every_variant() {
        let location = Span::new("source");
        let nodes = one_of_every_variant(location);

        // Every variant reports the location it was built with.
        for node in &nodes {
            assert_eq!(node.span(), location);
        }
    }

    #[test]
    fn derives_clone_debug_eq() {
        // Silly test to mark the `#[derive(...)]` lines across the module as
        // covered.
        let nodes = one_of_every_variant(Span::new("source"));
        let cloned = nodes.clone();

        assert_eq!(nodes, cloned);
        assert!(!format!("{nodes:?}").is_empty());
    }

    #[test]
    fn constructs_every_sub_variant() {
        // Exercise the remaining sub-enum variants not used above, so the whole
        // public vocabulary is instantiated at least once.
        let styles = [
            StyleVariant::Strong,
            StyleVariant::Emphasis,
            StyleVariant::Code,
            StyleVariant::Mark,
            StyleVariant::Superscript,
            StyleVariant::Subscript,
            StyleVariant::DoubleQuote,
            StyleVariant::SingleQuote,
            StyleVariant::Unquoted,
        ];

        let forms = [SpanForm::Constrained, SpanForm::Unconstrained];
        let refs = [RefVariant::Link, RefVariant::Xref];
        let stems = [StemNotation::AsciiMath, StemNotation::LatexMath];

        let char_refs = [
            CharRef::Special('&'),
            CharRef::Replacement("©"),
            CharRef::Entity(CowStr::from("&#8217;")),
        ];

        let ui_kinds = [
            UiKind::Keyboard(vec![CowStr::from("Ctrl"), CowStr::from("T")]),
            UiKind::Button(CowStr::from("OK")),
            UiKind::Menu {
                menu: CowStr::from("File"),
                submenus: vec![CowStr::from("Export")],
                item: Some(CowStr::from("PDF")),
            },
        ];

        let callout_guards = [
            CalloutGuard::LineComment(CowStr::from("// ")),
            CalloutGuard::Xml,
        ];

        // Touch each collection so the constructed values are observed.
        assert_eq!(styles.len(), 9);
        assert_eq!(forms.len(), 2);
        assert_eq!(refs.len(), 2);
        assert_eq!(stems.len(), 2);
        assert_eq!(char_refs.len(), 3);
        assert_eq!(ui_kinds.len(), 3);
        assert_eq!(callout_guards.len(), 2);
    }
}

/// Where a [`Raw`](InlineNode::Raw) node's content came from.
///
/// Orthogonal to [`RawForm`], which says what the fold *does* with the bytes.
/// This says who produced them, and it answers two different questions.
///
/// For a **consumer**, it sharpens the security story design §3.4 gives this
/// node kind. "This document emits raw HTML" is visible as `Raw` nodes in the
/// tree; whether that came from an author writing an explicit passthrough or
/// from a substitution expanding an attribute value is the difference between a
/// deliberate escape hatch and a value that may have arrived from elsewhere.
///
/// For the **builder**, it is the difference between content the extraction
/// pass is holding and content that is simply there. The string pipeline's own
/// haystack carries a sentinel where a [`Passthrough`](Self::Passthrough)
/// node's text belongs, and splices the real text back only when it rewrites
/// the rendered string — so a *computed value* that reads those bytes before
/// the restore (a cross-reference's target, captured into its deferred segment)
/// sees the sentinel, while one that reads a
/// [`Substitution`](Self::Substitution) node's bytes sees exactly what the
/// replacer saw. Recording it on the node is what lets a recognition gate tell
/// the two apart without having to know which pass it is running in.
///
/// A [`Passthrough`](Self::Passthrough) additionally carries the two facts the
/// extraction pass resolved for it, which nothing else in the tree records: the
/// substitution group its body is restored under, and — for the one form whose
/// [`value`](InlineNode::Raw) is *not* the author's own bytes — that body. See
/// the variant's own documentation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RawOrigin {
    /// An explicit passthrough the extraction pass pulled out before any step
    /// ran: `+++…+++`, `++…++`, `$$…$$`, `pass:[…]`, or an inline STEM body.
    ///
    /// The author asked for this text to be off limits.
    Passthrough {
        /// The substitution group this body is restored under, as the
        /// extraction pass resolved it — `None` for `+++…+++` and a bare
        /// `pass:[…]`, `Verbatim` for `++…++` and `$$…$$`, and whatever list a
        /// `pass:c,q[…]` spelled out.
        ///
        /// [`RawForm`] is the *fold's* two-valued view of this: a group that
        /// applies nothing renders [`AsIs`](RawForm::AsIs), one that applies
        /// special characters and nothing else renders
        /// [`Escaped`](RawForm::Escaped). The two agree for every group the
        /// fold can carry out by itself, and the group says more than the fold
        /// needs precisely so that a consumer asking *what the author wrote*
        /// does not have to infer it back from what the fold did.
        subs: SubstitutionGroup,

        /// The author's body **before** its group was applied, when
        /// [`value`](InlineNode::Raw) is not it.
        ///
        /// For every form the fold can restore by itself, `value` *is* the
        /// author's body and this is `None`. The exception is a `pass:` macro
        /// carrying an explicit substitution list (`pass:c,q[…]`): an arbitrary
        /// group needs the substitution pipeline, and a fold takes a renderer
        /// and a [`RenderContext`](crate::parser::RenderContext) rather than a
        /// `Parser`, so that body is substituted at **build** time and `value`
        /// holds the result. Recording the input beside it is what keeps the
        /// author's own text answerable from the tree.
        source_text: Option<String>,
    },

    /// Raw output a substitution produced **in place**, with no extraction
    /// involved: a literal `<`, `>`, or `&` from an expanded attribute value
    /// (which §3.4.1 leaves unescaped, since the value expands *after*
    /// `specialcharacters` ran), a special an effective order never escaped, or
    /// a slice of an entity's own bytes.
    Substitution,
}

/// How a [`Raw`](InlineNode::Raw) node's value reaches the output.
///
/// Both forms are **opaque**: no transducer step matches inside a `Raw` node,
/// whichever form it carries. What differs is only what the fold does with the
/// bytes — and, because the fold is where a renderer is chosen, that is exactly
/// the difference between a value that honors the renderer it is folded with
/// and one frozen against whatever renderer happened to be configured at parse
/// time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RawForm {
    /// `value` is already the bytes to emit, and the fold emits them unchanged.
    ///
    /// This is "raw output by design": a `+++…+++` or bare `pass:[…]` body
    /// (whose substitution group is [`None`], so nothing was applied), an
    /// entity's own bytes, or a literal special an effective order never
    /// escaped.
    ///
    /// [`None`]: crate::content::SubstitutionGroup
    AsIs,

    /// `value` is the author's *logical* text, and the fold escapes it exactly
    /// as it escapes a [`Text`](InlineNode::Text) node's.
    ///
    /// This is a body whose substitution group applies special characters and
    /// nothing else (`++…++`, `$$…$$`) — literal text that must not be
    /// *matched into* by a later step, but which is not raw output and must be
    /// escaped by whichever renderer the fold is given.
    Escaped,
}
