use crate::{
    HasSpan, Span,
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

    /// Verbatim, un-escaped output: passthrough content (`+++…+++`,
    /// `pass:[…]`, `$$…$$`) and any literal special character of an attribute
    /// expansion that the effective substitution order left unescaped. This
    /// node is the model's record of the language's "emit raw HTML by design"
    /// behavior. ASG: `inlineLiteral` with `name="raw"`.
    Raw {
        /// The content to emit verbatim, without HTML-escaping.
        value: CowStr<'src>,

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
            Anchor, Callout, CalloutGuard, CharRef, Footnote, Image, IndexTerm, InlineNode, Ref,
            RefVariant, SpanForm, Stem, StemNotation, StyleVariant, Styled, Ui, UiKind,
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
