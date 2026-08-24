use std::rc::Rc;

use crate::{
    document::InterpretedValue,
    parser::{
        ImageFileHandler, Parser, PathResolver, ResolvedAttributes, SafeMode, SvgFileHandler,
    },
};

/// The document state a renderer may read while it renders — everything an
/// [`InlineSubstitutionRenderer`] needs beyond the element's own
/// [`Attrlist`](crate::attributes::Attrlist), and nothing else.
///
/// A renderer used to receive the live [`Parser`] itself. That was more than it
/// needed and less than it could rely on: a `Parser`'s document attributes are
/// *mutable parse state*, so what a renderer read depended on **when** it ran.
/// While rendering happened inside the parse that was invisible, because "now"
/// and "the point in the document this element came from" were the same moment.
/// They stop being the same the instant any render runs later than its parse.
///
/// A `RenderContext` is a **snapshot**, so that class of question does not
/// arise: it reads what was true where it was taken, however the parser moves
/// on afterwards.
///
/// Taking one costs a handful of reference-count bumps and no allocation — the
/// attribute tables are shared from the parser by [`Arc`](std::sync::Arc) on
/// copy-on-write terms, and the resolver and file handlers by [`Rc`] — so a
/// caller that needs one per element is free to take one per element.
///
/// The lookups mirror the identically-named [`Parser`] methods exactly, so a
/// query here answers as the parser would have at the moment the context was
/// taken.
///
/// # Not `Clone`, deliberately
///
/// A context is something a renderer or handler is *handed*, for the duration
/// of one call. It is deliberately not clonable, so it cannot be retained
/// past that — which is what keeps it from forming a reference cycle: a
/// context holds an [`Rc`] to each file handler, so a handler that stored one
/// would own the thing that owns it, and neither would ever be freed. Taking
/// one is cheap enough that a caller who wants a context later should take a
/// fresh one instead of keeping this one.
///
/// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
#[derive(Debug)]
pub struct RenderContext {
    /// The document attributes, and the safe mode, as of the moment this
    /// context was taken.
    attributes: ResolvedAttributes,

    /// The resolver that turns an image or icon target into a web path.
    pub(crate) path_resolver: Rc<dyn PathResolver>,

    /// The handler that reads an image's bytes for a `data-uri` embed, if the
    /// parser had one.
    pub(crate) image_file_handler: Option<Rc<dyn ImageFileHandler>>,

    /// The handler that reads an SVG's contents for an inline embed, if the
    /// parser had one.
    pub(crate) svg_file_handler: Option<Rc<dyn SvgFileHandler>>,
}

impl RenderContext {
    /// Takes a context from `parser`'s current state.
    ///
    /// Visible only within [`parser`](crate::parser), so that
    /// [`Parser::render_context`] is the single way a context is built —
    /// see its docs for why that is crate-private.
    pub(super) fn new(parser: &Parser) -> Self {
        let mut attributes = parser.snapshot_attributes();

        // A snapshot resolves the time-dependent attributes lazily, capturing
        // afresh — right for an end-of-parse `Document`, wrong here. This
        // context is handed to a renderer *during* the parse, so a fresh
        // capture could report a different `{docdate}` than the content around
        // it was rendered with. Inherit the parse's own capture when it has
        // one.
        attributes.freeze_datetime(parser.captured_datetime_context());

        Self {
            attributes,
            path_resolver: Rc::clone(&parser.path_resolver),
            image_file_handler: parser.image_file_handler.clone(),
            svg_file_handler: parser.svg_file_handler.clone(),
        }
    }

    /// Builds a context pairing `attributes` — a snapshot taken at some
    /// earlier point in the parse — with `parser`'s configuration.
    ///
    /// See [`Parser::render_context_with`](crate::Parser), which is how this is
    /// reached, for why the two halves come from different places.
    pub(super) fn from_parts(attributes: ResolvedAttributes, parser: &Parser) -> Self {
        Self {
            attributes,
            path_resolver: Rc::clone(&parser.path_resolver),
            image_file_handler: parser.image_file_handler.clone(),
            svg_file_handler: parser.svg_file_handler.clone(),
        }
    }

    /// Returns the value of the document attribute named `name`, exactly as
    /// [`Parser::attribute_value`] would have answered when this context was
    /// taken.
    pub fn attribute_value<N: AsRef<str>>(&self, name: N) -> InterpretedValue {
        self.attributes.attribute_value(name)
    }

    /// Reports whether the document attribute named `name` has a value (or is
    /// set without one), exactly as [`Parser::has_attribute`] would have
    /// answered when this context was taken.
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        self.attributes.has_attribute(name)
    }

    /// Reports whether the document attribute named `name` is set, exactly as
    /// [`Parser::is_attribute_set`] would have answered when this context was
    /// taken.
    pub fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        self.attributes.is_attribute_set(name)
    }

    /// Returns the safe mode the parse ran under.
    pub fn safe_mode(&self) -> SafeMode {
        self.attributes.safe_mode()
    }

    /// Returns the [`PathResolver`] the parse was configured with — the one
    /// the built-in HTML backend resolves an image or icon target through.
    ///
    /// A custom [`InlineSubstitutionRenderer`] that resolves targets itself
    /// can use this to resolve them the same way, rather than reimplementing
    /// the resolution.
    ///
    /// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
    pub fn path_resolver(&self) -> &dyn PathResolver {
        self.path_resolver.as_ref()
    }

    /// Returns the [`ImageFileHandler`] the parse was configured with, if any.
    ///
    /// A custom [`InlineSubstitutionRenderer`] that resolves image URIs itself
    /// (rather than inheriting
    /// [`image_uri`](crate::parser::InlineSubstitutionRenderer::image_uri)'s
    /// default `data-uri` embedding) can use this to read an image's bytes
    /// through the same handler the built-in HTML renderer uses. `None` when
    /// no handler was registered, in which case there is no way to embed
    /// images and a web path should be used instead.
    ///
    /// This is the render-time counterpart of
    /// [`Parser::image_file_handler`](crate::Parser::image_file_handler): a
    /// renderer is handed a context rather than a parser, so this is how it
    /// reaches the handler.
    ///
    /// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
    pub fn image_file_handler(&self) -> Option<&dyn ImageFileHandler> {
        self.image_file_handler.as_deref()
    }

    /// Returns the [`SvgFileHandler`] the parse was configured with, if any.
    ///
    /// A custom [`InlineSubstitutionRenderer`] that renders inline SVG images
    /// itself (rather than inheriting
    /// [`render_image`](crate::parser::InlineSubstitutionRenderer::render_image)'s
    /// `opts=inline` handling) can use this to read an SVG's contents through
    /// the same handler the built-in HTML renderer uses. `None` when no
    /// handler was registered, in which case inline SVG contents are
    /// unavailable and the alt text should be used instead.
    ///
    /// This is the render-time counterpart of
    /// [`Parser::svg_file_handler`](crate::Parser::svg_file_handler): a
    /// renderer is handed a context rather than a parser, so this is how it
    /// reaches the handler.
    ///
    /// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
    pub fn svg_file_handler(&self) -> Option<&dyn SvgFileHandler> {
        self.svg_file_handler.as_deref()
    }
}

/// Serializes the tests that write `SOURCE_DATE_EPOCH`, which is process-wide
/// state the whole test binary shares.
#[cfg(test)]
static SOURCE_DATE_EPOCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::SOURCE_DATE_EPOCH_LOCK;
    use crate::{
        Parser,
        document::InterpretedValue,
        parser::{ModificationContext, SafeMode},
    };

    #[test]
    fn a_context_answers_each_lookup_as_the_parser_would() {
        let parser = Parser::default()
            .with_intrinsic_attribute("imagesdir", "img", ModificationContext::Anywhere)
            .with_intrinsic_attribute_bool("icons", true, ModificationContext::Anywhere)
            .with_safe_mode(SafeMode::Server);

        let context = parser.render_context();

        assert_eq!(
            context.attribute_value("imagesdir"),
            parser.attribute_value("imagesdir")
        );

        assert_eq!(
            context.attribute_value("imagesdir"),
            InterpretedValue::Value("img".to_string())
        );

        assert_eq!(context.is_attribute_set("icons"), true);
        assert_eq!(
            context.is_attribute_set("icons"),
            parser.is_attribute_set("icons")
        );

        assert_eq!(context.has_attribute("imagesdir"), true);
        assert_eq!(
            context.has_attribute("imagesdir"),
            parser.has_attribute("imagesdir")
        );

        assert_eq!(context.has_attribute("no-such-attribute"), false);
        assert_eq!(
            context.attribute_value("no-such-attribute"),
            InterpretedValue::Unset
        );

        assert_eq!(context.safe_mode(), SafeMode::Server);
        assert_eq!(context.safe_mode(), parser.safe_mode());
    }

    #[test]
    fn a_context_is_frozen_against_a_later_attribute_change() {
        // The property the type exists for, and the one a renderer that used
        // to hold the live `Parser` could not have. A document attribute is
        // *mutable parse state*: `:imagesdir:` rebinds it for everything after
        // it. A context taken where an element was written keeps answering for
        // that point, so a render running later than its parse cannot silently
        // pick up a value from further down the document.
        let parser = Parser::default();
        let before = parser.render_context();

        // `imagesdir` is a built-in that starts *set with no value* — an image
        // resolves against no directory — rather than unset.
        assert_eq!(before.attribute_value("imagesdir"), InterpretedValue::Set);

        let parser =
            parser.with_intrinsic_attribute("imagesdir", "img", ModificationContext::Anywhere);

        // The parser has moved on; the context has not.
        assert_eq!(
            parser.attribute_value("imagesdir"),
            InterpretedValue::Value("img".to_string())
        );

        assert_eq!(before.attribute_value("imagesdir"), InterpretedValue::Set);

        // And a context taken now does see it, so the freeze is per-context
        // rather than a snapshot that stopped tracking.
        assert_eq!(
            parser.render_context().attribute_value("imagesdir"),
            InterpretedValue::Value("img".to_string())
        );
    }

    #[test]
    fn a_context_inherits_the_parses_own_datetime_capture() {
        // A snapshot resolves `docdate` and its family *lazily*, capturing the
        // clock afresh on each read. That is right for an end-of-parse
        // `Document` snapshot and wrong for a context, which is handed to a
        // renderer during the parse: a fresh capture there can report a
        // different day than the content around it was rendered with — the
        // exact class of "depends on when it runs" this type exists to remove.
        //
        // `SOURCE_DATE_EPOCH` is what makes the clock movable deterministically
        // (no waiting for midnight). Serialized against the other test that
        // sets it, since the environment is process-wide.
        let _guard = SOURCE_DATE_EPOCH_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 2020-01-02, then 2021-06-07.
        //
        // SAFETY: the lock above serializes every test in this crate that
        // writes this variable, and no other thread reads it concurrently.
        unsafe { std::env::set_var("SOURCE_DATE_EPOCH", "1577923200") };

        let parser = Parser::default();

        // Reading it through the parser is what captures and caches the
        // instant — as rendering content that mentions `{docdate}` would.
        assert_eq!(
            parser.attribute_value("docdate"),
            InterpretedValue::Value("2020-01-02".to_string())
        );

        // SAFETY: as above.
        unsafe { std::env::set_var("SOURCE_DATE_EPOCH", "1623024000") };

        // The parser stays on the captured day...
        assert_eq!(
            parser.attribute_value("docdate"),
            InterpretedValue::Value("2020-01-02".to_string())
        );

        // ...and so must a context taken from it. Before the fix this read
        // `2021-06-07`.
        assert_eq!(
            parser.render_context().attribute_value("docdate"),
            InterpretedValue::Value("2020-01-02".to_string())
        );

        // A parser that has captured *nothing* keeps the lazy path: there is
        // no already-rendered value to be consistent with, so a context reads
        // the clock as it stands rather than forcing a capture per element.
        let fresh = Parser::default();

        assert_eq!(
            fresh.render_context().attribute_value("docdate"),
            InterpretedValue::Value("2021-06-07".to_string())
        );

        // SAFETY: as above.
        unsafe { std::env::remove_var("SOURCE_DATE_EPOCH") };
    }

    #[test]
    fn a_context_carries_the_parsers_resolver_and_handlers() {
        // The three `Rc`s beyond the attribute tables: a renderer resolves web
        // paths and embeds file contents through them, so a context that
        // dropped them would silently change how every image renders.
        let parser = Parser::default();
        let context = parser.render_context();

        assert!(Rc::ptr_eq(&context.path_resolver, &parser.path_resolver));
        assert!(context.image_file_handler.is_none());
        assert!(context.svg_file_handler.is_none());

        // The default resolver is always present, and reaching it through the
        // accessor resolves a path the same way — an accessor wired to the
        // wrong field would answer differently rather than merely compare
        // unequal.
        assert_eq!(
            context.path_resolver().web_path("b.png", Some("img")),
            "img/b.png"
        );
    }
}
