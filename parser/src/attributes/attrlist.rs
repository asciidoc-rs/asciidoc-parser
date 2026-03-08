use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::{ElementAttribute, element_attribute::ParseShorthand},
    content::{Content, SubstitutionStep},
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// The source text that’s used to define attributes for an element is referred
/// to as an **attrlist.** An attrlist is always enclosed in a pair of square
/// brackets. This applies for block attributes as well as attributes on a block
/// or inline macro. The processor splits the attrlist into individual attribute
/// entries, determines whether each entry is a positional or named attribute,
/// parses the entry accordingly, and assigns the result as an attribute on the
/// node.
#[derive(Clone, Eq, PartialEq, Default)]
pub struct Attrlist<'src> {
    attributes: Vec<ElementAttribute<'src>>,
    anchor: Option<CowStr<'src>>,
    source: Span<'src>,
}

impl<'src> Attrlist<'src> {
    /// **IMPORTANT:** This `source` span passed to this function should NOT
    /// include the opening or closing square brackets for the attrlist.
    /// This is because the rules for closing brackets differ when parsing
    /// inline, macro, and block elements.
    pub(crate) fn parse(
        source: Span<'src>,
        parser: &Parser,
        attrlist_context: AttrlistContext,
    ) -> MatchAndWarnings<'src, MatchedItem<'src, Self>> {
        let mut attributes: Vec<ElementAttribute> = vec![];
        let mut parse_shorthand_items = true;
        let mut warnings: Vec<Warning<'src>> = vec![];

        // Apply attribute value substitutions before parsing attrlist content.
        let source_cow = if source.contains('{') && source.contains('}') {
            let mut content = Content::from(source);
            SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);
            CowStr::from(content.rendered.to_string())
        } else {
            CowStr::from(source.data())
        };

        if source_cow.starts_with('[') && source_cow.ends_with(']') {
            let anchor = source_cow[1..source_cow.len() - 1].to_owned();

            return MatchAndWarnings {
                item: MatchedItem {
                    item: Self {
                        attributes,
                        anchor: Some(CowStr::from(anchor)),
                        source,
                    },
                    after: source.discard_all(),
                },
                warnings,
            };
        }

        let mut index = 0;

        let after_index = loop {
            let (attr, new_index, warning_types) = ElementAttribute::parse(
                &source_cow,
                index,
                parser,
                ParseShorthand(parse_shorthand_items),
                attrlist_context,
            );

            // Because we do attribute value substitution early on in parsing, we can't
            // pinpoint the exact location of warnings in an attribute list. For that
            // reason, individual attribute parsing only returns the warning type and we
            // then map it back to the entire attrlist source.
            for warning_type in warning_types {
                warnings.push(Warning {
                    source,
                    warning: warning_type,
                });
            }

            if attr.name().is_none() {
                parse_shorthand_items = false;
            }

            let mut after = Span::new(source_cow.as_ref()).discard(new_index);

            if attr.name().is_none()
                && attr.value().is_empty()
                && after.is_empty()
                && attributes.is_empty()
            {
                break index;
            }

            if attr.name().is_none() || attr.value() != "None" {
                attributes.push(attr);
            }

            after = after.take_whitespace().after;

            match after.take_prefix(",") {
                Some(comma) => {
                    after = comma.after.take_whitespace().after;

                    if after.starts_with(',') {
                        warnings.push(Warning {
                            source,
                            warning: WarningType::EmptyAttributeValue,
                        });
                        after = after.discard(1);
                        index = after.byte_offset();
                        continue;
                    }

                    index = after.byte_offset();
                }
                None => {
                    break after.byte_offset();
                }
            }
        };

        if after_index < source_cow.len() {
            warnings.push(Warning {
                source,
                warning: WarningType::MissingCommaAfterQuotedAttributeValue,
            });
        }

        MatchAndWarnings {
            item: MatchedItem {
                item: Self {
                    attributes,
                    anchor: None,
                    source,
                },
                after: source.discard_all(),
            },
            warnings,
        }
    }

    /// Returns an iterator over the attributes contained within
    /// this attrlist.
    pub fn attributes(&'src self) -> Iter<'src, ElementAttribute<'src>> {
        self.attributes.iter()
    }

    /// Returns the anchor found in this attribute list, if any.
    pub fn anchor(&'src self) -> Option<&'src str> {
        self.anchor.as_deref()
    }

    /// Returns the first attribute with the given name.
    pub fn named_attribute(&'src self, name: &str) -> Option<&'src ElementAttribute<'src>> {
        self.attributes.iter().find(|attr| {
            if let Some(attr_name) = attr.name() {
                attr_name == name
            } else {
                false
            }
        })
    }

    /// Returns the given (1-based) positional attribute.
    ///
    /// **IMPORTANT:** Named attributes with names are disregarded when
    /// counting.
    pub fn nth_attribute(&'src self, n: usize) -> Option<&'src ElementAttribute<'src>> {
        if n == 0 {
            None
        } else {
            self.attributes
                .iter()
                .filter(|attr| attr.name().is_none())
                .nth(n - 1)
        }
    }

    /// Returns the first attribute with the given name or (1-based) index.
    ///
    /// Some block and macro types provide implicit mappings between attribute
    /// names and positions to permit a shorthand syntax.
    ///
    /// This method will search by name first, and fall back to positional
    /// indexing if the name doesn't yield a match.
    pub fn named_or_positional_attribute(
        &'src self,
        name: &str,
        index: usize,
    ) -> Option<&'src ElementAttribute<'src>> {
        self.named_attribute(name)
            .or_else(|| self.nth_attribute(index))
    }

    /// Returns the ID attribute (if any).
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
    pub fn id(&'src self) -> Option<&'src str> {
        self.anchor().or_else(|| {
            self.nth_attribute(1)
                .and_then(|attr1| attr1.id())
                .or_else(|| self.named_attribute("id").map(|attr| attr.value()))
        })
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
    pub fn roles(&'src self) -> Vec<&'src str> {
        let mut roles = self
            .nth_attribute(1)
            .map(|attr1| attr1.roles())
            .unwrap_or_default();

        if let Some(role_attr) = self.named_attribute("role") {
            let mut role_span = Span::new(role_attr.value());
            let mut formal_roles: Vec<&'src str> = vec![];
            role_span = role_span.take_while(|c| c == ' ').after;

            while !role_span.is_empty() {
                let mi = role_span.take_while(|c| c != ' ');
                if !mi.item.is_empty() {
                    formal_roles.push(mi.item.data());
                }
                role_span = mi.after.take_while(|c| c == ' ').after;
            }

            roles.append(&mut formal_roles);
        }

        roles
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
    /// You can assign multiple options to a block by prefixing each value with
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
    pub fn options(&'src self) -> Vec<&'src str> {
        let mut options = self
            .nth_attribute(1)
            .map(|attr1| attr1.options())
            .unwrap_or_default();

        if let Some(option_attr) = self.named_attribute("opts") {
            let mut option_span = Span::new(option_attr.value());
            let mut formal_options: Vec<&'src str> = vec![];
            option_span = option_span.take_while(|c| c == ',').after;

            while !option_span.is_empty() {
                let mi = option_span.take_while(|c| c != ',');
                if !mi.item.is_empty() {
                    formal_options.push(mi.item.data());
                }
                option_span = mi.after.take_while(|c| c == ',').after;
            }

            options.append(&mut formal_options);
        }

        if let Some(option_attr) = self.named_attribute("options") {
            let mut option_span = Span::new(option_attr.value());
            let mut formal_options: Vec<&'_ str> = vec![];
            option_span = option_span.take_while(|c| c == ',').after;

            while !option_span.is_empty() {
                let mi = option_span.take_while(|c| c != ',');
                if !mi.item.is_empty() {
                    formal_options.push(mi.item.data());
                }
                option_span = mi.after.take_while(|c| c == ',').after;
            }

            options.append(&mut formal_options);
        }

        options
    }

    /// Returns `true` if this attribute list has the named option.
    ///
    /// See [`options()`] for a description of option syntax.
    ///
    /// [`options()`]: Self::options
    pub fn has_option<N: AsRef<str>>(&'src self, name: N) -> bool {
        // PERF: Might help to optimize away the construction of the options Vec.
        let options = self.options();
        let name = name.as_ref();
        options.contains(&name)
    }

    /// Return the block style name from shorthand syntax.
    pub fn block_style(&'src self) -> Option<&'src str> {
        self.nth_attribute(1).and_then(|a| a.block_style())
    }
}

impl<'src> HasSpan<'src> for Attrlist<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for Attrlist<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attrlist")
            .field("attributes", &DebugSliceReference(&self.attributes))
            .field("anchor", &self.anchor)
            .field("source", &self.source)
            .finish()
    }
}

/// Context for attribute list parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttrlistContext {
    Block,
    Inline,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{attributes::AttrlistContext, tests::prelude::*};

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let p = Parser::default();
        let b1 = crate::attributes::Attrlist::parse(
            crate::Span::new("abc"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        let b2 = b1.item.clone();
        assert_eq!(b1.item, b2);
    }

    #[test]
    fn impl_default() {
        let attrlist = crate::attributes::Attrlist::default();

        assert_eq!(
            attrlist,
            Attrlist {
                attributes: &[],
                anchor: None,
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(attrlist.named_attribute("foo").is_none());

        assert!(attrlist.nth_attribute(0).is_none());
        assert!(attrlist.nth_attribute(1).is_none());
        assert!(attrlist.nth_attribute(42).is_none());

        assert!(attrlist.named_or_positional_attribute("foo", 0).is_none());
        assert!(attrlist.named_or_positional_attribute("foo", 1).is_none());
        assert!(attrlist.named_or_positional_attribute("foo", 42).is_none());

        assert!(attrlist.id().is_none());
        assert!(attrlist.roles().is_empty());
        assert!(attrlist.block_style().is_none());

        assert_eq!(
            attrlist.span(),
            Span {
                data: "",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn empty_source() {
        let p = Parser::default();

        let mi =
            crate::attributes::Attrlist::parse(crate::Span::default(), &p, AttrlistContext::Inline)
                .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[],
                anchor: None,
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());

        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.nth_attribute(1).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 1).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 42).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.block_style().is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 1,
                offset: 0
            }
        );
    }

    #[test]
    fn empty_positional_attributes() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new(",300,400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: ""
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: ",300,400",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.block_style().is_none());

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: ""
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: ""
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.named_or_positional_attribute("height", 4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: ",300,400",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 9,
                offset: 8
            }
        );
    }

    #[test]
    fn only_positional_attributes() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("Sunset,300,400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["Sunset"],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "Sunset,300,400",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert_eq!(mi.item.block_style().unwrap(), "Sunset");

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.named_or_positional_attribute("height", 4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "Sunset,300,400",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 15,
                offset: 14
            }
        );
    }

    #[test]
    fn trim_trailing_space() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("Sunset ,300 , 400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["Sunset"],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "Sunset ,300 , 400",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert_eq!(mi.item.block_style().unwrap(), "Sunset");

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.named_or_positional_attribute("height", 4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "Sunset ,300 , 400",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 18,
                offset: 17
            }
        );
    }

    #[test]
    fn only_named_attributes() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("alt=Sunset,width=300,height=400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: Some("alt"),
                        shorthand_items: &[],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: Some("width"),
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: Some("height"),
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "alt=Sunset,width=300,height=400",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert_eq!(
            mi.item.named_attribute("alt").unwrap(),
            ElementAttribute {
                name: Some("alt"),
                shorthand_items: &[],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: Some("alt"),
                shorthand_items: &[],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_attribute("width").unwrap(),
            ElementAttribute {
                name: Some("width"),
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: Some("width"),
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_attribute("height").unwrap(),
            ElementAttribute {
                name: Some("height"),
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 3).unwrap(),
            ElementAttribute {
                name: Some("height"),
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.nth_attribute(1).is_none());
        assert!(mi.item.nth_attribute(2).is_none());
        assert!(mi.item.nth_attribute(3).is_none());
        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.block_style().is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "alt=Sunset,width=300,height=400",
                line: 1,
                col: 1,
                offset: 0
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 32,
                offset: 31
            }
        );
    }

    #[test]
    fn ignore_named_attribute_with_none_value() {
        let p = Parser::default();
        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("alt=Sunset,width=None,height=400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: Some("alt"),
                        shorthand_items: &[],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: Some("height"),
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "alt=Sunset,width=None,height=400",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert_eq!(
            mi.item.named_attribute("alt").unwrap(),
            ElementAttribute {
                name: Some("alt"),
                shorthand_items: &[],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: Some("alt"),
                shorthand_items: &[],
                value: "Sunset"
            }
        );

        assert!(mi.item.named_attribute("width").is_none());
        assert!(mi.item.named_or_positional_attribute("width", 2).is_none());

        assert_eq!(
            mi.item.named_attribute("height").unwrap(),
            ElementAttribute {
                name: Some("height"),
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 2).unwrap(),
            ElementAttribute {
                name: Some("height"),
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.nth_attribute(1).is_none());
        assert!(mi.item.nth_attribute(2).is_none());
        assert!(mi.item.nth_attribute(3).is_none());
        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.block_style().is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "alt=Sunset,width=None,height=400",
                line: 1,
                col: 1,
                offset: 0
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 33,
                offset: 32
            }
        );
    }

    #[test]
    fn err_unparsed_remainder_after_value() {
        let p = Parser::default();

        let maw = crate::attributes::Attrlist::parse(
            crate::Span::new("alt=\"Sunset\"width=300"),
            &p,
            AttrlistContext::Inline,
        );

        let mi = maw.item.clone();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[ElementAttribute {
                    name: Some("alt"),
                    shorthand_items: &[],
                    value: "Sunset"
                }],
                anchor: None,
                source: Span {
                    data: "alt=\"Sunset\"width=300",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 22,
                offset: 21
            }
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "alt=\"Sunset\"width=300",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warning: WarningType::MissingCommaAfterQuotedAttributeValue,
            }]
        );
    }

    #[test]
    fn propagates_error_from_element_attribute() {
        let p = Parser::default();

        let maw = crate::attributes::Attrlist::parse(
            crate::Span::new("foo%#id"),
            &p,
            AttrlistContext::Inline,
        );

        let mi = maw.item.clone();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["foo", "#id"],
                    value: "foo%#id"
                }],
                anchor: None,
                source: Span {
                    data: "foo%#id",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 8,
                offset: 7
            }
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "foo%#id",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warning: WarningType::EmptyShorthandItem,
            }]
        );
    }

    #[test]
    fn anchor_syntax() {
        let p = Parser::default();

        let maw = crate::attributes::Attrlist::parse(
            crate::Span::new("[notice]"),
            &p,
            AttrlistContext::Inline,
        );

        let mi = maw.item.clone();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[],
                anchor: Some("notice"),
                source: Span {
                    data: "[notice]",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 9,
                offset: 8
            }
        );

        assert!(maw.warnings.is_empty());
    }

    mod id {
        use crate::{attributes::AttrlistContext, tests::prelude::*};

        #[test]
        fn via_shorthand_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("#goals"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["#goals"],
                        value: "#goals"
                    }],
                    anchor: None,
                    source: Span {
                        data: "#goals",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());
            assert_eq!(mi.item.id().unwrap(), "goals");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.item.span(),
                Span {
                    data: "#goals",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }

        #[test]
        fn via_named_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,id=goals"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("id"),
                            shorthand_items: &[],
                            value: "goals"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,id=goals",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("id").unwrap(),
                ElementAttribute {
                    name: Some("id"),
                    shorthand_items: &[],
                    value: "goals"
                }
            );

            assert_eq!(mi.item.id().unwrap(), "goals");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 17,
                    offset: 16
                }
            );
        }

        #[test]
        fn via_block_anchor_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("[goals]"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[],
                    anchor: Some("goals"),
                    source: Span {
                        data: "[goals]",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(mi.item.id().unwrap(), "goals");

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 8,
                    offset: 7
                }
            );
        }

        #[test]
        fn shorthand_only_first_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo,blah#goals"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["foo"],
                            value: "foo"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "blah#goals"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo,blah#goals",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert_eq!(mi.item.block_style().unwrap(), "foo");

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 15,
                    offset: 14
                }
            );
        }
    }

    mod roles {
        use crate::{attributes::AttrlistContext, tests::prelude::*};

        #[test]
        fn via_shorthand_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new(".rolename"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &[".rolename"],
                        value: ".rolename"
                    }],
                    anchor: None,
                    source: Span {
                        data: ".rolename",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"rolename");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.item.span(),
                Span {
                    data: ".rolename",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 10,
                    offset: 9
                }
            );
        }

        #[test]
        fn via_shorthand_syntax_trim_trailing_whitespace() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new(".rolename "),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &[".rolename"],
                        value: ".rolename"
                    }],
                    anchor: None,
                    source: Span {
                        data: ".rolename ",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let roles = mi.item.roles();
            let mut roles = roles.iter();

            assert_eq!(roles.next().unwrap(), &"rolename");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.item.span(),
                Span {
                    data: ".rolename ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 11,
                    offset: 10
                }
            );
        }

        #[test]
        fn multiple_roles_via_shorthand_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new(".role1.role2.role3"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &[".role1", ".role2", ".role3"],
                        value: ".role1.role2.role3"
                    }],
                    anchor: None,
                    source: Span {
                        data: ".role1.role2.role3",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"role1");
            assert_eq!(roles.next().unwrap(), &"role2");
            assert_eq!(roles.next().unwrap(), &"role3");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.item.span(),
                Span {
                    data: ".role1.role2.role3",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 19,
                    offset: 18
                }
            );
        }

        #[test]
        fn multiple_roles_via_shorthand_syntax_trim_whitespace() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new(".role1 .role2 .role3 "),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &[".role1", ".role2", ".role3"],
                        value: ".role1 .role2 .role3"
                    }],
                    anchor: None,
                    source: Span {
                        data: ".role1 .role2 .role3 ",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"role1");
            assert_eq!(roles.next().unwrap(), &"role2");
            assert_eq!(roles.next().unwrap(), &"role3");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.item.span(),
                Span {
                    data: ".role1 .role2 .role3 ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 22,
                    offset: 21
                }
            );
        }

        #[test]
        fn via_named_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,role=role1"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("role"),
                            shorthand_items: &[],
                            value: "role1"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,role=role1",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("role").unwrap(),
                ElementAttribute {
                    name: Some("role"),
                    shorthand_items: &[],
                    value: "role1"
                }
            );

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"role1");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 19,
                    offset: 18
                }
            );
        }

        #[test]
        fn multiple_roles_via_named_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,role=role1 role2   role3 "),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("role"),
                            shorthand_items: &[],
                            value: "role1 role2   role3"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,role=role1 role2   role3 ",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("role").unwrap(),
                ElementAttribute {
                    name: Some("role"),
                    shorthand_items: &[],
                    value: "role1 role2   role3"
                }
            );

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"role1");
            assert_eq!(roles.next().unwrap(), &"role2");
            assert_eq!(roles.next().unwrap(), &"role3");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 34,
                    offset: 33
                }
            );
        }

        #[test]
        fn shorthand_role_and_named_attribute_role() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("#foo.sh1.sh2,role=na1 na2   na3 "),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["#foo", ".sh1", ".sh2"],
                            value: "#foo.sh1.sh2"
                        },
                        ElementAttribute {
                            name: Some("role"),
                            shorthand_items: &[],
                            value: "na1 na2   na3"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "#foo.sh1.sh2,role=na1 na2   na3 ",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());

            assert_eq!(
                mi.item.named_attribute("role").unwrap(),
                ElementAttribute {
                    name: Some("role"),
                    shorthand_items: &[],
                    value: "na1 na2   na3"
                }
            );

            let roles = mi.item.roles();
            let mut roles = roles.iter();
            assert_eq!(roles.next().unwrap(), &"sh1");
            assert_eq!(roles.next().unwrap(), &"sh2");
            assert_eq!(roles.next().unwrap(), &"na1");
            assert_eq!(roles.next().unwrap(), &"na2");
            assert_eq!(roles.next().unwrap(), &"na3");
            assert!(roles.next().is_none());

            assert!(mi.item.block_style().is_none());

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 33,
                    offset: 32
                }
            );
        }

        #[test]
        fn shorthand_only_first_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo,blah.rolename"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["foo"],
                            value: "foo"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "blah.rolename"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo,blah.rolename",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            let roles = mi.item.roles();
            assert_eq!(roles.iter().len(), 0);

            assert_eq!(mi.item.block_style().unwrap(), "foo");

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 18,
                    offset: 17
                }
            );
        }
    }

    mod options {
        use crate::{attributes::AttrlistContext, tests::prelude::*};

        #[test]
        fn via_shorthand_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("%option"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%option"],
                        value: "%option"
                    }],
                    anchor: None,
                    source: Span {
                        data: "%option",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"option",);
            assert!(options.next().is_none());

            assert!(mi.item.has_option("option"));
            assert!(!mi.item.has_option("option1"));

            assert_eq!(
                mi.item.span(),
                Span {
                    data: "%option",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 8,
                    offset: 7
                }
            );
        }

        #[test]
        fn multiple_options_via_shorthand_syntax() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("%option1%option2%option3"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%option1", "%option2", "%option3",],
                        value: "%option1%option2%option3"
                    }],
                    anchor: None,
                    source: Span {
                        data: "%option1%option2%option3",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none());
            assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"option1");
            assert_eq!(options.next().unwrap(), &"option2");
            assert_eq!(options.next().unwrap(), &"option3");
            assert!(options.next().is_none());

            assert!(mi.item.has_option("option1"));
            assert!(mi.item.has_option("option2"));
            assert!(mi.item.has_option("option3"));
            assert!(!mi.item.has_option("option4"));

            assert_eq!(
                mi.item.span(),
                Span {
                    data: "%option1%option2%option3",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 25,
                    offset: 24
                }
            );
        }

        #[test]
        fn via_options_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,options=option1"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("options"),
                            shorthand_items: &[],
                            value: "option1"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,options=option1",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("options").unwrap(),
                ElementAttribute {
                    name: Some("options"),
                    shorthand_items: &[],
                    value: "option1"
                }
            );

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"option1");
            assert!(options.next().is_none());

            assert!(mi.item.has_option("option1"));
            assert!(!mi.item.has_option("option2"));

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 24,
                    offset: 23
                }
            );
        }

        #[test]
        fn via_opts_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,opts=option1"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("opts"),
                            shorthand_items: &[],
                            value: "option1"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,opts=option1",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("opts").unwrap(),
                ElementAttribute {
                    name: Some("opts"),
                    shorthand_items: &[],
                    value: "option1"
                }
            );

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"option1");
            assert!(options.next().is_none());

            assert!(!mi.item.has_option("option"));
            assert!(mi.item.has_option("option1"));
            assert!(!mi.item.has_option("option2"));

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 21,
                    offset: 20
                }
            );
        }

        #[test]
        fn multiple_options_via_named_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo=bar,options=\"option1,option2,option3\""),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: Some("foo"),
                            shorthand_items: &[],
                            value: "bar"
                        },
                        ElementAttribute {
                            name: Some("options"),
                            shorthand_items: &[],
                            value: "option1,option2,option3"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo=bar,options=\"option1,option2,option3\"",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert_eq!(
                mi.item.named_attribute("foo").unwrap(),
                ElementAttribute {
                    name: Some("foo"),
                    shorthand_items: &[],
                    value: "bar"
                }
            );

            assert_eq!(
                mi.item.named_attribute("options").unwrap(),
                ElementAttribute {
                    name: Some("options"),
                    shorthand_items: &[],
                    value: "option1,option2,option3"
                }
            );

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"option1");
            assert_eq!(options.next().unwrap(), &"option2");
            assert_eq!(options.next().unwrap(), &"option3");
            assert!(options.next().is_none());

            assert!(mi.item.has_option("option1"));
            assert!(mi.item.has_option("option2"));
            assert!(mi.item.has_option("option3"));
            assert!(!mi.item.has_option("option4"));

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 42,
                    offset: 41
                }
            );
        }

        #[test]
        fn shorthand_option_and_named_attribute_option() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("#foo%sh1%sh2,options=\"na1,na2,na3\""),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["#foo", "%sh1", "%sh2"],
                            value: "#foo%sh1%sh2"
                        },
                        ElementAttribute {
                            name: Some("options"),
                            shorthand_items: &[],
                            value: "na1,na2,na3"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "#foo%sh1%sh2,options=\"na1,na2,na3\"",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            assert!(mi.item.named_attribute("foo").is_none(),);

            assert_eq!(
                mi.item.named_attribute("options").unwrap(),
                ElementAttribute {
                    name: Some("options"),
                    shorthand_items: &[],
                    value: "na1,na2,na3"
                }
            );

            let options = mi.item.options();
            let mut options = options.iter();
            assert_eq!(options.next().unwrap(), &"sh1");
            assert_eq!(options.next().unwrap(), &"sh2");
            assert_eq!(options.next().unwrap(), &"na1");
            assert_eq!(options.next().unwrap(), &"na2");
            assert_eq!(options.next().unwrap(), &"na3");
            assert!(options.next().is_none(),);

            assert!(mi.item.has_option("sh1"));
            assert!(mi.item.has_option("sh2"));
            assert!(!mi.item.has_option("sh3"));
            assert!(mi.item.has_option("na1"));
            assert!(mi.item.has_option("na2"));
            assert!(mi.item.has_option("na3"));
            assert!(!mi.item.has_option("na4"));

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 35,
                    offset: 34
                }
            );
        }

        #[test]
        fn shorthand_only_first_attribute() {
            let p = Parser::default();

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("foo,blah%option"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["foo"],
                            value: "foo"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "blah%option"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "foo,blah%option",
                        line: 1,
                        col: 1,
                        offset: 0
                    }
                }
            );

            let options = mi.item.options();
            assert_eq!(options.iter().len(), 0);

            assert!(!mi.item.has_option("option"));

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 16,
                    offset: 15
                }
            );
        }
    }

    #[test]
    fn block_style() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("blah#goals"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        let attrlist = mi.item;
        assert_eq!(attrlist.block_style().unwrap(), "blah");
    }

    #[test]
    fn err_double_comma() {
        let p = Parser::default();

        let maw = crate::attributes::Attrlist::parse(
            crate::Span::new("alt=Sunset,width=300,,height=400"),
            &p,
            AttrlistContext::Inline,
        );

        let mi = maw.item.clone();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: Some("alt"),
                        shorthand_items: &[],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: Some("width"),
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: Some("height"),
                        shorthand_items: &[],
                        value: "400"
                    },
                ],
                anchor: None,
                source: Span {
                    data: "alt=Sunset,width=300,,height=400",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 33,
                offset: 32,
            }
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "alt=Sunset,width=300,,height=400",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warning: WarningType::EmptyAttributeValue,
            }]
        );
    }

    #[test]
    fn applies_attribute_substitution_before_parsing() {
        let p = Parser::default().with_intrinsic_attribute(
            "sunset_dimensions",
            "300,400",
            ModificationContext::Anywhere,
        );

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("Sunset,{sunset_dimensions}"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["Sunset"],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "Sunset,{sunset_dimensions}",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert_eq!(mi.item.block_style().unwrap(), "Sunset");

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "300"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("height", 3).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "400"
            }
        );

        assert!(mi.item.nth_attribute(4).is_none());
        assert!(mi.item.named_or_positional_attribute("height", 4).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "Sunset,{sunset_dimensions}",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 27,
                offset: 26,
            }
        );
    }

    #[test]
    fn ignores_unknown_attribute_when_applying_attribution_substitution() {
        let p = Parser::default().with_intrinsic_attribute(
            "sunset_dimensions",
            "300,400",
            ModificationContext::Anywhere,
        );

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("Sunset,{not_sunset_dimensions}"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["Sunset"],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "{not_sunset_dimensions}"
                    },
                ],
                anchor: None,
                source: Span {
                    data: "Sunset,{not_sunset_dimensions}",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());
        assert!(mi.item.named_or_positional_attribute("foo", 0).is_none());

        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert_eq!(mi.item.block_style().unwrap(), "Sunset");

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "{not_sunset_dimensions}"
            }
        );

        assert_eq!(
            mi.item.named_or_positional_attribute("width", 2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "{not_sunset_dimensions}"
            }
        );

        assert!(mi.item.nth_attribute(3).is_none());
        assert!(mi.item.named_or_positional_attribute("height", 3).is_none());
        assert!(mi.item.nth_attribute(42).is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: "Sunset,{not_sunset_dimensions}",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 31,
                offset: 30,
            }
        );
    }

    #[test]
    fn impl_debug() {
        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new("Sunset,300,400"),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        let attrlist = mi.item;

        assert_eq!(
            format!("{attrlist:#?}"),
            r#"Attrlist {
    attributes: &[
        ElementAttribute {
            name: None,
            value: "Sunset",
            shorthand_item_indices: [
                0,
            ],
        },
        ElementAttribute {
            name: None,
            value: "300",
            shorthand_item_indices: [],
        },
        ElementAttribute {
            name: None,
            value: "400",
            shorthand_item_indices: [],
        },
    ],
    anchor: None,
    source: Span {
        data: "Sunset,300,400",
        line: 1,
        col: 1,
        offset: 0,
    },
}"#
        );
    }
}
