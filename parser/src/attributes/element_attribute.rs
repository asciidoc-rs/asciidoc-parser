use crate::{
    Parser, Span,
    attributes::AttrlistContext,
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    strings::CowStr,
    warnings::WarningType,
};

/// This struct represents a single element attribute.
///
/// Element attributes define the built-in and user-defined settings and
/// metadata that can be applied to an individual block element or inline
/// element in a document (including macros). Although the include directive is
/// not technically an element, element attributes can also be defined on an
/// include directive.
#[derive(Clone, Eq, PartialEq)]
pub struct ElementAttribute<'src> {
    name: Option<CowStr<'src>>,
    value: CowStr<'src>,
    shorthand_item_indices: Vec<usize>,

    /// `true` when the normal substitution group was already applied to `value`
    /// while parsing this attribute (a single-quoted value in a block attribute
    /// list). Consumers that would otherwise substitute the value themselves —
    /// such as deriving a block title from a `title=` attribute — use this to
    /// avoid substituting it a second time (which would, e.g., double-escape
    /// special characters).
    value_is_substituted: bool,
}

impl std::fmt::Debug for ElementAttribute<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `value_is_substituted` is an internal parsing detail and is
        // deliberately omitted so the debug representation stays focused on the
        // attribute's observable content.
        f.debug_struct("ElementAttribute")
            .field("name", &self.name)
            .field("value", &self.value)
            .field("shorthand_item_indices", &self.shorthand_item_indices)
            .finish()
    }
}

impl<'src> ElementAttribute<'src> {
    pub(crate) fn parse(
        source_text: &CowStr<'src>,
        start_index: usize,
        parser: &Parser,
        mut parse_shorthand: ParseShorthand,
        attrlist_context: AttrlistContext,
    ) -> (Self, usize, Vec<WarningType>) {
        let mut warnings: Vec<WarningType> = vec![];

        let (name, value, shorthand_item_indices, value_is_substituted, offset) = {
            let mut source = Span::new(source_text.as_ref());
            source = source.discard(start_index);

            let (name, after): (Option<Span<'_>>, Span) = match source.take_attr_name() {
                Some(name) => {
                    let space = name.after.take_whitespace_with_newline();
                    match space.after.take_prefix("=") {
                        Some(equals) => {
                            let space = equals.after.take_whitespace_with_newline();
                            // `name=` with nothing (or only a comma) after the `=`
                            // is a named attribute with an empty value, not a
                            // positional one. The empty value falls out of the
                            // value scan below (`take_while(c != ',')` yields the
                            // empty string), so the name is all we need to keep.
                            (Some(name.item), space.after)
                        }
                        None => (None, source),
                    }
                }
                None => (None, source),
            };

            let after = after.take_whitespace_with_newline().after;
            let first_char = after.data().chars().next();

            let value = match first_char {
                Some('\'') | Some('"') => match after.take_quoted_string() {
                    Some(v) => {
                        parse_shorthand = ParseShorthand(false);
                        v
                    }
                    None => {
                        warnings.push(WarningType::AttributeValueMissingTerminatingQuote);
                        after.take_while(|c| c != ',').trim_item_trailing_spaces()
                    }
                },
                _ => after.take_while(|c| c != ',').trim_item_trailing_spaces(),
            };

            let after = value.after;
            let mut value = cowstr_from_source_and_span(source_text, &value.item);
            let mut value_is_substituted = false;

            if let Some(first) = first_char
                && (first == '\'' || first == '\"')
            {
                let escaped_quote = format!("\\{first}");
                let mut new_value = value.replace(&escaped_quote, &first.to_string());

                if first == '\'' && attrlist_context == AttrlistContext::Block {
                    // A single-quoted value in a block attribute list has the
                    // normal substitution group applied at assignment time.
                    value_is_substituted = true;

                    let span = Span::new(&new_value);
                    let mut content = Content::from(span);
                    SubstitutionGroup::Normal.apply(&mut content, parser, None);

                    if content.rendered.as_ref() != new_value {
                        new_value = content.rendered.to_string();
                    }
                }

                if new_value != *value {
                    value = CowStr::from(new_value);
                }
            }

            let shorthand_item_indices = if name.is_none() && parse_shorthand.0 {
                parse_shorthand_items(&value, &mut warnings)
            } else {
                vec![]
            };

            let name = name.map(|name| cowstr_from_source_and_span(source_text, &name));

            (
                name,
                value,
                shorthand_item_indices,
                value_is_substituted,
                after.byte_offset(),
            )
        };

        (
            Self {
                name,
                value,
                shorthand_item_indices,
                value_is_substituted,
            },
            offset,
            warnings,
        )
    }

    /// Synthesize the `source` block-style attribute implied by a
    /// language-aware fenced code block (`` ```lang ``). This is the first
    /// positional attribute of the equivalent `[source,<lang>]` attribute list;
    /// its shorthand items resolve the block style to `source`.
    pub(crate) fn synthesized_source_style() -> Self {
        const SOURCE: &str = "source";
        let mut warnings: Vec<WarningType> = vec![];
        let shorthand_item_indices = parse_shorthand_items(SOURCE, &mut warnings);

        // `source` is a single shorthand item with no delimiters, so parsing it
        // can never warn. Guard that invariant rather than plumb an always-empty
        // list back to the caller.
        debug_assert!(
            warnings.is_empty(),
            "synthesizing the `source` block style should not produce warnings, got: {warnings:?}"
        );

        Self {
            name: None,
            value: CowStr::from(SOURCE),
            shorthand_item_indices,
            value_is_substituted: false,
        }
    }

    /// Construct a bare positional attribute whose value is drawn directly from
    /// `span`, carrying no shorthand items. Used for the language on a
    /// language-aware fenced code block (the second positional attribute of the
    /// equivalent `[source,<lang>]` attribute list).
    pub(crate) fn positional_from_span(span: Span<'src>) -> Self {
        Self {
            name: None,
            value: CowStr::from(span.data()),
            shorthand_item_indices: vec![],
            value_is_substituted: false,
        }
    }

    /// Return the attribute name, if one was found.
    pub fn name(&'src self) -> Option<&'src str> {
        self.name_str()
    }

    /// Return the attribute name, if one was found.
    ///
    /// Unlike [`name`](Self::name), this borrows for the duration of the call
    /// only, so it can be used on temporary `ElementAttribute` values (for
    /// example while merging multiple attribute lists).
    pub(crate) fn name_str(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the shorthand items, if applicable.
    ///
    /// Shorthand items are only parsed for certain element attributes. If this
    /// attribute is not of the appropriate kind, this will return an empty
    /// list.
    pub fn shorthand_items(&'src self) -> Vec<&'src str> {
        self.shorthand_items_internal()
    }

    /// Same as [`shorthand_items`](Self::shorthand_items), but borrows for the
    /// duration of the call only so it can be used on temporary values.
    fn shorthand_items_internal(&self) -> Vec<&str> {
        let mut result = vec![];
        let value = self.value.as_ref();

        let mut iter = self.shorthand_item_indices.iter().peekable();

        while let Some(curr) = iter.next() {
            let mut next_item = if let Some(next) = iter.peek() {
                &value[*curr..**next]
            } else {
                &value[*curr..]
            };

            if next_item == "#" || next_item == "." || next_item == "%" {
                continue;
            }

            next_item = next_item.trim_end();

            if !next_item.is_empty() {
                result.push(next_item);
            }
        }

        result
    }

    /// Return the block style name from shorthand syntax.
    pub fn block_style(&'src self) -> Option<&'src str> {
        self.block_style_internal()
    }

    /// Same as [`block_style`](Self::block_style), but borrows for the duration
    /// of the call only.
    fn block_style_internal(&self) -> Option<&str> {
        self.shorthand_items_internal()
            .first()
            .filter(|v| !v.chars().any(is_shorthand_delimiter))
            .cloned()
    }

    /// Return the ID attribute from shorthand syntax.
    ///
    /// If multiple ID attributes were specified, only the first
    /// match is returned. (Multiple IDs are not supported.)
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
        self.id_internal()
    }

    /// Same as [`id`](Self::id), but borrows for the duration of the call only.
    fn id_internal(&self) -> Option<&str> {
        self.shorthand_items_internal()
            .into_iter()
            .find(|v| v.starts_with('#'))
            .map(|v| &v[1..])
    }

    /// Return any role attributes that were found in shorthand syntax.
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
        self.roles_internal()
    }

    /// Same as [`roles`](Self::roles), but borrows for the duration of the call
    /// only.
    fn roles_internal(&self) -> Vec<&str> {
        self.shorthand_items_internal()
            .into_iter()
            .filter(|span| span.starts_with('.'))
            .map(|span| &span[1..])
            .collect()
    }

    /// Return any option attributes that were found in shorthand syntax.
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
    pub fn options(&'src self) -> Vec<&'src str> {
        self.options_internal()
    }

    /// Same as [`options`](Self::options), but borrows for the duration of the
    /// call only.
    fn options_internal(&self) -> Vec<&str> {
        self.shorthand_items_internal()
            .into_iter()
            .filter(|v| v.starts_with('%'))
            .map(|v| &v[1..])
            .collect()
    }

    /// Merge the shorthand of two first-position (block style) attributes drawn
    /// from separate block attribute lines, following Asciidoctor's semantics:
    ///
    /// * the block style and ID are taken from the later line if it specifies
    ///   them, otherwise retained from the earlier line;
    /// * roles and options accumulate, with the earlier line's values first.
    ///
    /// The result is a freshly synthesized positional attribute whose value
    /// re-encodes the merged shorthand so that the usual accessors continue to
    /// work.
    pub(crate) fn merge_block_style_shorthand(earlier: &Self, later: &Self) -> Self {
        let block_style = later
            .block_style_internal()
            .or(earlier.block_style_internal());
        let id = later.id_internal().or(earlier.id_internal());

        let mut roles = earlier.roles_internal();
        roles.extend(later.roles_internal());

        let mut options = earlier.options_internal();
        options.extend(later.options_internal());

        let mut value = String::new();
        if let Some(block_style) = block_style {
            value.push_str(block_style);
        }
        if let Some(id) = id {
            value.push('#');
            value.push_str(id);
        }
        for role in &roles {
            value.push('.');
            value.push_str(role);
        }
        for option in &options {
            value.push('%');
            value.push_str(option);
        }

        // The shorthand string is synthesized entirely from components that
        // were already parsed and validated when their source lines were read
        // (a block style, a non-empty ID, and non-empty roles/options, each
        // separated by a single delimiter). Re-parsing it therefore can never
        // produce a warning, so rather than plumb an always-empty warning list
        // back to the caller we assert that invariant — turning a silent
        // discard into an explicit, regression-guarded one.
        let mut warnings: Vec<WarningType> = vec![];
        let shorthand_item_indices = if value.is_empty() {
            vec![]
        } else {
            parse_shorthand_items(&value, &mut warnings)
        };

        debug_assert!(
            warnings.is_empty(),
            "merging block-style shorthand should not produce warnings, got: {warnings:?}"
        );

        Self {
            name: None,
            value: CowStr::from(value),
            shorthand_item_indices,
            value_is_substituted: false,
        }
    }

    /// Return the attribute's value.
    ///
    /// Note that this value will have had special characters and attribute
    /// value replacements applied to it.
    pub fn value(&'src self) -> &'src str {
        self.value.as_ref()
    }

    /// Return the attribute's value.
    ///
    /// Unlike [`value`](Self::value), this borrows for the duration of the call
    /// only, so it can be used on temporary `ElementAttribute` values (for
    /// example while an owned attribute list is still being assembled).
    pub(crate) fn value_str(&self) -> &str {
        self.value.as_ref()
    }

    /// Returns `true` when the normal substitution group has already been
    /// applied to this attribute's [`value`](Self::value) (a single-quoted
    /// value in a block attribute list). A consumer that would otherwise
    /// substitute the value itself uses this to avoid substituting it a
    /// second time.
    pub(crate) fn value_is_substituted(&self) -> bool {
        self.value_is_substituted
    }
}

fn parse_shorthand_items(source: &str, warnings: &mut Vec<WarningType>) -> Vec<usize> {
    let mut shorthand_item_indices: Vec<usize> = vec![];
    let mut span = Span::new(source);

    // Look for block style selector.
    if let Some(block_style_pr) = span.split_at_match_non_empty(is_shorthand_delimiter) {
        shorthand_item_indices.push(block_style_pr.item.discard_whitespace().byte_offset());

        span = block_style_pr.after;
    }

    while !span.is_empty() {
        // Assumption: First character is a delimiter.
        let after_delimiter = span.discard(1);

        match after_delimiter.position(is_shorthand_delimiter) {
            None => {
                if after_delimiter.is_empty() {
                    warnings.push(WarningType::EmptyShorthandItem);
                    shorthand_item_indices.push(span.byte_offset());
                    span = after_delimiter;
                } else {
                    shorthand_item_indices.push(span.byte_offset());
                    span = span.discard_all();
                }
            }

            Some(0) => {
                shorthand_item_indices.push(span.byte_offset());
                warnings.push(WarningType::EmptyShorthandItem);
                span = after_delimiter;
            }

            Some(index) => {
                let mi: MatchedItem<Span> = span.into_parse_result(index + 1);
                shorthand_item_indices.push(span.byte_offset());
                span = mi.after;
            }
        }
    }

    shorthand_item_indices
}

fn is_shorthand_delimiter(c: char) -> bool {
    c == '#' || c == '%' || c == '.'
}

#[derive(Clone, Debug)]
pub(crate) struct ParseShorthand(pub bool);

fn cowstr_from_source_and_span<'src>(source: &CowStr<'src>, span: &Span<'_>) -> CowStr<'src> {
    if let CowStr::Borrowed(source) = source {
        let borrowed: Span<'src> = Span::new(source)
            .discard(span.byte_offset())
            .slice_to(..span.len());

        CowStr::Borrowed(borrowed.data())
    } else {
        CowStr::from(span.data().to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{
        attributes::{AttrlistContext, element_attribute::ParseShorthand},
        strings::CowStr,
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let p = Parser::default();

        let b1 = crate::attributes::ElementAttribute::parse(
            &CowStr::from("abc"),
            0,
            &p,
            ParseShorthand(false),
            AttrlistContext::Inline,
        )
        .0;

        let b2 = b1.clone();

        assert_eq!(b1, b2);
    }

    #[test]
    fn empty_source() {
        let p = Parser::default();

        let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
            &CowStr::from(""),
            0,
            &p,
            ParseShorthand(false),
            AttrlistContext::Inline,
        );

        assert!(warning_types.is_empty());

        assert_eq!(
            element_attr,
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "",
            }
        );

        assert!(element_attr.name().is_none());
        assert!(element_attr.block_style().is_none());
        assert!(element_attr.id().is_none());
        assert!(element_attr.roles().is_empty());
        assert!(element_attr.options().is_empty());

        assert_eq!(offset, 0);
    }

    #[test]
    fn only_spaces() {
        let p = Parser::default();

        let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
            &CowStr::from("   "),
            0,
            &p,
            ParseShorthand(false),
            AttrlistContext::Inline,
        );

        assert!(warning_types.is_empty());

        assert_eq!(
            element_attr,
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "",
            }
        );

        assert!(element_attr.name().is_none());
        assert!(element_attr.block_style().is_none());
        assert!(element_attr.id().is_none());
        assert!(element_attr.roles().is_empty());
        assert!(element_attr.options().is_empty());

        assert_eq!(offset, 3);
    }

    #[test]
    fn unquoted_and_unnamed_value() {
        let p = Parser::default();

        let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
            &CowStr::from("abc"),
            0,
            &p,
            ParseShorthand(false),
            AttrlistContext::Inline,
        );

        assert!(warning_types.is_empty());

        assert_eq!(
            element_attr,
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "abc",
            }
        );

        assert!(element_attr.name().is_none());
        assert!(element_attr.block_style().is_none());
        assert!(element_attr.id().is_none());
        assert!(element_attr.roles().is_empty());
        assert!(element_attr.options().is_empty());

        assert_eq!(offset, 3);
    }

    #[test]
    fn unquoted_stops_at_comma() {
        let p = Parser::default();

        let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
            &CowStr::from("abc,def"),
            0,
            &p,
            ParseShorthand(false),
            AttrlistContext::Inline,
        );

        assert!(warning_types.is_empty());

        assert_eq!(
            element_attr,
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "abc",
            }
        );

        assert!(element_attr.name().is_none());
        assert!(element_attr.block_style().is_none());
        assert!(element_attr.id().is_none());
        assert!(element_attr.roles().is_empty());
        assert!(element_attr.options().is_empty());

        assert_eq!(offset, 3);
    }

    mod quoted_string {
        use crate::{
            attributes::{AttrlistContext, element_attribute::ParseShorthand},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn err_unterminated_double_quote() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\"xyz"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "\"xyz"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);

            assert_eq!(
                warning_types,
                vec![WarningType::AttributeValueMissingTerminatingQuote]
            );
        }

        #[test]
        fn err_unterminated_double_quote_ends_at_comma() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\"xyz,abc"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "\"xyz"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);
            assert_eq!(
                warning_types,
                vec![WarningType::AttributeValueMissingTerminatingQuote]
            );
        }

        #[test]
        fn double_quoted_string() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\"abc\"def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "abc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 5);
        }

        #[test]
        fn double_quoted_with_escape() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\"a\\\"bc\"def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "a\"bc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 7);
        }

        #[test]
        fn double_quoted_with_single_quote() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\"a'bc\"def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "a'bc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 6);
        }

        #[test]
        fn err_unterminated_single_quote() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\'xyz"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "\'xyz"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);

            assert_eq!(
                warning_types,
                vec![WarningType::AttributeValueMissingTerminatingQuote]
            );
        }

        #[test]
        fn err_unterminated_single_quote_ends_at_comma() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("\'xyz,abc"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "\'xyz"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);
            assert_eq!(
                warning_types,
                vec![WarningType::AttributeValueMissingTerminatingQuote]
            );
        }

        #[test]
        fn single_quoted_string() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("'abc'def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "abc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 5);
        }

        #[test]
        fn single_quoted_with_escape() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("'a\\'bc'def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "a'bc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 7);
        }

        #[test]
        fn single_quoted_with_double_quote() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("'a\"bc'def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "a\"bc"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 6);
        }

        #[test]
        fn single_quoted_gets_substitions() {
            let p = Parser::default().with_intrinsic_attribute(
                "foo",
                "bar",
                ModificationContext::Anywhere,
            );

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("'*abc* def {foo}'"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Block,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "<strong>abc</strong> def bar"
                }
            );

            assert!(element_attr.name().is_none());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 17);
        }
    }

    mod named {
        use crate::{
            attributes::{AttrlistContext, element_attribute::ParseShorthand},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn simple_named_value() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc=def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("abc"),
                    shorthand_items: &[],
                    value: "def"
                }
            );

            assert_eq!(element_attr.name().unwrap(), "abc");
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 7);
        }

        #[test]
        fn ignores_spaces_around_equals() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc =  def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("abc"),
                    shorthand_items: &[],
                    value: "def"
                }
            );

            assert_eq!(element_attr.name().unwrap(), "abc");

            assert_eq!(offset, 10);
        }

        #[test]
        fn numeric_name() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("94-x =def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("94-x"),
                    shorthand_items: &[],
                    value: "def"
                }
            );

            assert_eq!(element_attr.name().unwrap(), "94-x");
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 9);
        }

        #[test]
        fn quoted_value() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc='def'g"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("abc"),
                    shorthand_items: &[],
                    value: "def"
                }
            );

            assert_eq!(element_attr.name().unwrap(), "abc");
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 9);
        }

        #[test]
        fn named_with_empty_value() {
            let p = Parser::default();

            // `name=` with nothing after the `=` is a named attribute whose value
            // is the empty string (e.g. `[caption=]` clears a label), not a
            // positional attribute with the literal value "abc=".
            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc="),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("abc"),
                    shorthand_items: &[],
                    value: ""
                }
            );

            assert_eq!(element_attr.name(), Some("abc"));
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);
        }

        #[test]
        fn named_with_empty_value_before_comma() {
            let p = Parser::default();

            // `name=` immediately followed by a comma is likewise a named
            // attribute with an empty value; parsing stops at the comma so the
            // next attribute can be read separately.
            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc=,def"),
                0,
                &p,
                ParseShorthand(false),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("abc"),
                    shorthand_items: &[],
                    value: ""
                }
            );

            assert_eq!(element_attr.name(), Some("abc"));
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);
        }
    }

    mod parse_with_shorthand {
        use crate::{
            attributes::{AttrlistContext, element_attribute::ParseShorthand},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn block_style_only() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["abc"],
                    value: "abc"
                }
            );

            assert!(element_attr.name().is_none());
            assert_eq!(element_attr.shorthand_items(), vec!["abc"]);
            assert_eq!(element_attr.block_style().unwrap(), "abc");
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 3);
        }

        #[test]
        fn ignore_if_named_attribute() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("name=block_style#id"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: Some("name"),
                    shorthand_items: &[],
                    value: "block_style#id"
                }
            );

            assert_eq!(element_attr.name().unwrap(), "name");
            assert!(element_attr.shorthand_items().is_empty());
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 19);
        }

        #[test]
        fn error_empty_id() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc#"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["abc"],
                    value: "abc#"
                }
            );

            assert_eq!(offset, 4);
            assert_eq!(warning_types, vec![WarningType::EmptyShorthandItem]);
        }

        #[test]
        fn error_duplicate_delimiter() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("abc##id"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["abc", "#id"],
                    value: "abc##id"
                }
            );

            assert_eq!(offset, 7);
            assert_eq!(warning_types, vec![WarningType::EmptyShorthandItem]);
        }

        #[test]
        fn id_only() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("#xyz"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["#xyz"],
                    value: "#xyz"
                }
            );

            assert!(element_attr.name().is_none());
            assert_eq!(element_attr.shorthand_items(), vec!["#xyz"]);
            assert!(element_attr.block_style().is_none());
            assert_eq!(element_attr.id().unwrap(), "xyz");
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 4);
        }

        #[test]
        fn one_role_only() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from(".role1"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[".role1",],
                    value: ".role1"
                }
            );

            assert!(element_attr.name().is_none());
            assert_eq!(element_attr.shorthand_items(), vec![".role1"]);
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert_eq!(element_attr.roles(), vec!("role1"));
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 6);
        }

        #[test]
        fn multiple_roles() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from(".role1.role2.role3"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[".role1", ".role2", ".role3"],
                    value: ".role1.role2.role3"
                }
            );

            assert!(element_attr.name().is_none());

            assert_eq!(
                element_attr.shorthand_items(),
                vec![".role1", ".role2", ".role3"]
            );

            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert_eq!(element_attr.roles(), vec!("role1", "role2", "role3",));
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 18);
        }

        #[test]
        fn one_option_only() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("%option1"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["%option1"],
                    value: "%option1"
                }
            );

            assert!(element_attr.name().is_none());
            assert_eq!(element_attr.shorthand_items(), vec!["%option1"]);
            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert_eq!(element_attr.options(), vec!("option1"));

            assert_eq!(offset, 8);
        }

        #[test]
        fn multiple_options() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("%option1%option2%option3"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["%option1", "%option2", "%option3"],
                    value: "%option1%option2%option3"
                }
            );

            assert!(element_attr.name().is_none());

            assert_eq!(
                element_attr.shorthand_items(),
                vec!["%option1", "%option2", "%option3"]
            );

            assert!(element_attr.block_style().is_none());
            assert!(element_attr.id().is_none());
            assert!(element_attr.roles().is_empty());
            assert_eq!(
                element_attr.options(),
                vec!("option1", "option2", "option3")
            );

            assert_eq!(offset, 24);
        }

        #[test]
        fn block_style_and_id() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("appendix#custom-id"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["appendix", "#custom-id"],
                    value: "appendix#custom-id"
                }
            );

            assert!(element_attr.name().is_none());
            assert_eq!(
                element_attr.shorthand_items(),
                vec!["appendix", "#custom-id"]
            );
            assert_eq!(element_attr.block_style().unwrap(), "appendix",);
            assert_eq!(element_attr.id().unwrap(), "custom-id",);
            assert!(element_attr.roles().is_empty());
            assert!(element_attr.options().is_empty());

            assert_eq!(offset, 18);
        }

        #[test]
        fn id_role_and_option() {
            let p = Parser::default();

            let (element_attr, offset, warning_types) = crate::attributes::ElementAttribute::parse(
                &CowStr::from("#rules.prominent%incremental"),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Inline,
            );

            assert!(warning_types.is_empty());

            assert_eq!(
                element_attr,
                ElementAttribute {
                    name: None,
                    shorthand_items: &["#rules", ".prominent", "%incremental"],
                    value: "#rules.prominent%incremental"
                }
            );

            assert!(element_attr.name().is_none());

            assert_eq!(
                element_attr.shorthand_items(),
                vec!["#rules", ".prominent", "%incremental"]
            );

            assert!(element_attr.block_style().is_none());
            assert_eq!(element_attr.id().unwrap(), "rules");
            assert_eq!(element_attr.roles(), vec!("prominent"));
            assert_eq!(element_attr.options(), vec!("incremental"));

            assert_eq!(offset, 28);
        }
    }

    mod merge_block_style_shorthand {
        use crate::{
            attributes::{AttrlistContext, element_attribute::ParseShorthand},
            strings::CowStr,
            tests::prelude::*,
        };

        fn parse(value: &str) -> crate::attributes::ElementAttribute<'_> {
            let p = Parser::default();
            crate::attributes::ElementAttribute::parse(
                &CowStr::from(value),
                0,
                &p,
                ParseShorthand(true),
                AttrlistContext::Block,
            )
            .0
        }

        #[test]
        fn merges_disjoint_shorthand() {
            let earlier = parse("#myid");
            let later = parse(".myrole");

            let merged =
                crate::attributes::ElementAttribute::merge_block_style_shorthand(&earlier, &later);

            assert!(merged.name().is_none());
            assert_eq!(merged.id().unwrap(), "myid");
            assert_eq!(merged.roles(), vec!["myrole"]);
        }

        #[test]
        fn two_empty_positionals_stay_empty() {
            // Merging two empty first positionals exercises the empty-value
            // branch, which produces an attribute with no shorthand items.
            let earlier = parse("");
            let later = parse("");

            let merged =
                crate::attributes::ElementAttribute::merge_block_style_shorthand(&earlier, &later);

            assert_eq!(
                merged,
                ElementAttribute {
                    name: None,
                    shorthand_items: &[],
                    value: "",
                }
            );

            assert!(merged.name().is_none());
            assert!(merged.block_style().is_none());
            assert!(merged.id().is_none());
            assert!(merged.roles().is_empty());
            assert!(merged.options().is_empty());
        }
    }
}
