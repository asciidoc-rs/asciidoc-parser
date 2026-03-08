use std::{fmt::Debug, slice::Iter};

use crate::{
    Span,
    attributes::Attrlist,
    blocks::{Block, is_built_in_context},
    content::SubstitutionGroup,
    strings::CowStr,
};

/// **Block elements** form the main structure of an AsciiDoc document, starting
/// with the document itself.
///
/// A block element (aka **block**) is a discrete, line-oriented chunk of
/// content in an AsciiDoc document. Once parsed, that chunk of content becomes
/// a block element in the parsed document model. Certain blocks may contain
/// other blocks, so we say that blocks can be nested. The converter visits each
/// block in turn, in document order, converting it to a corresponding chunk of
/// output.
///
/// This trait implements many of the same core methods as the [`Block`] enum
/// but provides a mechanism for third-party code to extend the behavior of
/// blocks.
pub trait IsBlock<'src>: Debug + Eq + PartialEq {
    /// Returns the [`ContentModel`] for this block.
    fn content_model(&self) -> ContentModel;

    /// Returns the rendered content for this block, if any.
    ///
    /// Some blocks (especially compound blocks) do not directly contain
    /// content. In such cases, this function will return `None`.
    ///
    /// This content will contain the text _after_ substitutions have been
    /// applied.
    fn rendered_content(&'src self) -> Option<&'src str> {
        None
    }

    /// Returns the resolved context for this block.
    ///
    /// A block’s context is also sometimes referred to as a name, such as an
    /// example block, a sidebar block, an admonition block, or a section.
    ///
    /// Every block has a context. The context is often implied by the syntax,
    /// but can be declared explicitly in certain cases. The context is what
    /// distinguishes one kind of block from another. You can think of the
    /// context as the block’s type.
    ///
    /// For that reason, the context is not defined as an enumeration, but
    /// rather as a string type that is optimized for the case where predefined
    /// constants are viable.
    ///
    /// A block's context can be replaced by a block style that matches a
    /// built-in context. Unlike [`raw_context()`], that transformation _is_
    /// performed by this function.
    ///
    /// [`raw_context()`]: Self::raw_context
    fn resolved_context(&'src self) -> CowStr<'src> {
        if let Some(declared_style) = self.declared_style()
            && is_built_in_context(declared_style)
        {
            return declared_style.into();
        }

        self.raw_context()
    }

    /// Returns the raw (uninterpreted) context for this block.
    ///
    /// A block’s context is also sometimes referred to as a name, such as an
    /// example block, a sidebar block, an admonition block, or a section.
    ///
    /// Every block has a context. The context is often implied by the syntax,
    /// but can be declared explicitly in certain cases. The context is what
    /// distinguishes one kind of block from another. You can think of the
    /// context as the block’s type.
    ///
    /// For that reason, the context is not defined as an enumeration, but
    /// rather as a string type that is optimized for the case where predefined
    /// constants are viable.
    ///
    /// A block's context can be replaced by a block style that matches a
    /// built-in context. That transformation is only performed by
    /// [`resolved_context()`], not this function.
    ///
    /// [`resolved_context()`]: Self::resolved_context
    fn raw_context(&self) -> CowStr<'src>;

    /// Returns the declared (uninterpreted) style for this block.
    ///
    /// Above some blocks, you may notice a name at the start of the block
    /// attribute list (e.g., `[source]` or `[verse]`). The first positional
    /// (unnamed) attribute in the block attribute list is used to declare the
    /// block style.
    ///
    /// The declared block style is the value the author supplies.
    ///
    /// That value is then interpreted and resolved. That interpretation is not
    /// performed by this function.
    fn declared_style(&'src self) -> Option<&'src str> {
        self.attrlist()
            .and_then(|attrlist| attrlist.nth_attribute(1))
            .and_then(|attr| attr.block_style())
    }

    /// Returns an iterator over the nested blocks contained within
    /// this block.
    ///
    /// Many block types do not have nested blocks so the default implementation
    /// returns an empty iterator.
    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        const NO_BLOCKS: &[Block<'static>] = &[];
        NO_BLOCKS.iter()
    }

    /// Returns the ID for this block, if present.
    ///
    /// You can assign an ID to a block using the shorthand syntax, the longhand
    /// syntax, or a legacy block anchor.
    ///
    /// In the shorthand syntax, you prefix the name with a hash (`#`) in the
    /// first position attribute:
    ///
    /// ```asciidoc
    /// [#goals]
    /// * Goal 1
    /// * Goal 2
    /// ```
    ///
    /// In the longhand syntax, you use a standard named attribute:
    ///
    /// ```asciidoc
    /// [id=goals]
    /// * Goal 1
    /// * Goal 2
    /// ```
    ///
    /// In the legacy block anchor syntax, you surround the name with double
    /// square brackets:
    ///
    /// ```asciidoc
    /// [[goals]]
    /// * Goal 1
    /// * Goal 2
    /// ```
    fn id(&'src self) -> Option<&'src str> {
        self.anchor()
            .map(|a| a.data())
            .or_else(|| self.attrlist().and_then(|attrlist| attrlist.id()))
    }

    /// Returns any role attributes that were found.
    ///
    /// You can assign one or more roles to blocks and most inline elements
    /// using the `role` attribute. The `role` attribute is a [named attribute].
    /// Even though the attribute name is singular, it may contain multiple
    /// (space-separated) roles. Roles may also be defined using a shorthand
    /// (dot-prefixed) syntax.
    ///
    /// A role:
    /// 1. adds additional semantics to an element
    /// 2. can be used to apply additional styling to a group of elements (e.g.,
    ///    via a CSS class selector)
    /// 3. may activate additional behavior if recognized by the converter
    ///
    /// **TIP:** The `role` attribute in AsciiDoc always get mapped to the
    /// `class` attribute in the HTML output. In other words, role names are
    /// synonymous with HTML class names, thus allowing output elements to be
    /// identified and styled in CSS using class selectors (e.g.,
    /// `sidebarblock.role1`).
    ///
    /// [named attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/positional-and-named-attributes/#named
    fn roles(&'src self) -> Vec<&'src str> {
        match self.attrlist() {
            Some(attrlist) => attrlist.roles(),
            None => vec![],
        }
    }

    /// Returns any option attributes that were found.
    ///
    /// The `options` attribute (often abbreviated as `opts`) is a versatile
    /// [named attribute] that can be assigned one or more values. It can be
    /// defined globally as document attribute as well as a block attribute on
    /// an individual block.
    ///
    /// There is no strict schema for options. Any options which are not
    /// recognized are ignored.
    ///
    /// You can assign one or more options to a block using the shorthand or
    /// formal syntax for the options attribute.
    ///
    /// # Shorthand options syntax for blocks
    ///
    /// To assign an option to a block, prefix the value with a percent sign
    /// (`%`) in an attribute list. The percent sign implicitly sets the
    /// `options` attribute.
    ///
    /// ## Example 1: Sidebar block with an option assigned using the shorthand dot
    ///
    /// ```asciidoc
    /// [%option]
    /// ****
    /// This is a sidebar with an option assigned to it, named option.
    /// ****
    /// ```
    ///
    /// You can assign multiple options to a block by prest
    /// fixing each value with
    /// a percent sign (`%`).
    ///
    /// ## Example 2: Sidebar with two options assigned using the shorthand dot
    /// ```asciidoc
    /// [%option1%option2]
    /// ****
    /// This is a sidebar with two options assigned to it, named option1 and option2.
    /// ****
    /// ```
    ///
    /// # Formal options syntax for blocks
    ///
    /// Explicitly set `options` or `opts`, followed by the equals sign (`=`),
    /// and then the value in an attribute list.
    ///
    /// ## Example 3. Sidebar block with an option assigned using the formal syntax
    /// ```asciidoc
    /// [opts=option]
    /// ****
    /// This is a sidebar with an option assigned to it, named option.
    /// ****
    /// ```
    ///
    /// Separate multiple option values with commas (`,`).
    ///
    /// ## Example 4. Sidebar with three options assigned using the formal syntax
    /// ```asciidoc
    /// [opts="option1,option2"]
    /// ****
    /// This is a sidebar with two options assigned to it, option1 and option2.
    /// ****
    /// ```
    ///
    /// [named attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/positional-and-named-attributes/#named
    fn options(&'src self) -> Vec<&'src str> {
        match self.attrlist() {
            Some(attrlist) => attrlist.options(),
            None => vec![],
        }
    }

    /// Returns `true` if this block has the named option.
    ///
    /// See [`options()`] for a description of option syntax.
    ///
    /// [`options()`]: Self::options
    fn has_option<N: AsRef<str>>(&'src self, name: N) -> bool {
        self.attrlist()
            .is_some_and(|attrlist| attrlist.has_option(name))
    }

    /// Returns the source text for the title for this block, if present.
    fn title_source(&'src self) -> Option<Span<'src>>;

    /// Returns the rendered title for this block, if present.
    fn title(&self) -> Option<&str>;

    /// Returns the anchor for this block, if present.
    fn anchor(&'src self) -> Option<Span<'src>>;

    /// Returns the reference text for this block's anchor, if present.
    fn anchor_reftext(&'src self) -> Option<Span<'src>>;

    /// Returns the attribute list for this block, if present.
    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>>;

    /// Returns the default substitution group that is applied unless you
    /// customize the substitutions for a particular element.
    fn substitution_group(&'src self) -> SubstitutionGroup {
        SubstitutionGroup::Normal
    }
}

/// The content model of a block determines what kind of content the block can
/// have (if any) and how that content is processed.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ContentModel {
    /// A block that may only contain other blocks (e.g., a section)
    Compound,

    /// A block that's treated as contiguous lines of paragraph text (and
    /// subject to normal substitutions) (e.g., a paragraph block)
    Simple,

    /// A block that holds verbatim text (displayed "as is") (and subject to
    /// verbatim substitutions) (e.g., a listing block)
    Verbatim,

    /// A block that holds unprocessed content passed directly through to the
    /// output with no substitutions applied (e.g., a passthrough block)
    Raw,

    /// A block that has no content (e.g., an image block)
    Empty,

    /// A special content model reserved for tables that enforces a fixed
    /// structure
    Table,
}

impl std::fmt::Debug for ContentModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentModel::Compound => write!(f, "ContentModel::Compound"),
            ContentModel::Simple => write!(f, "ContentModel::Simple"),
            ContentModel::Verbatim => write!(f, "ContentModel::Verbatim"),
            ContentModel::Raw => write!(f, "ContentModel::Raw"),
            ContentModel::Empty => write!(f, "ContentModel::Empty"),
            ContentModel::Table => write!(f, "ContentModel::Table"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod content_model {
        mod impl_debug {
            use crate::blocks::ContentModel;

            #[test]
            fn compound() {
                let content_model = ContentModel::Compound;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Compound");
            }

            #[test]
            fn simple() {
                let content_model = ContentModel::Simple;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Simple");
            }

            #[test]
            fn verbatim() {
                let content_model = ContentModel::Verbatim;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Verbatim");
            }

            #[test]
            fn raw() {
                let content_model = ContentModel::Raw;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Raw");
            }

            #[test]
            fn empty() {
                let content_model = ContentModel::Empty;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Empty");
            }

            #[test]
            fn table() {
                let content_model = ContentModel::Table;
                let debug_output = format!("{:?}", content_model);
                assert_eq!(debug_output, "ContentModel::Table");
            }
        }
    }
}
