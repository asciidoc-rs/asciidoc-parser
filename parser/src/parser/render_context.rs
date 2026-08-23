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
/// [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer
#[derive(Clone, Debug)]
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
    pub(crate) fn new(parser: &Parser) -> Self {
        Self {
            attributes: parser.snapshot_attributes(),
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
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

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
    fn a_context_carries_the_parsers_resolver_and_handlers() {
        // The three `Rc`s beyond the attribute tables: a renderer resolves web
        // paths and embeds file contents through them, so a context that
        // dropped them would silently change how every image renders.
        let parser = Parser::default();
        let context = parser.render_context();

        assert!(Rc::ptr_eq(&context.path_resolver, &parser.path_resolver));
        assert!(context.image_file_handler.is_none());
        assert!(context.svg_file_handler.is_none());
    }
}
