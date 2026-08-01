use crate::{
    HasSpan, Parser, Span,
    attributes::{ElementAttribute, element_attribute::ParseShorthand},
    content::{Content, SubstitutionStep},
    internal::{debug::DebugSliceReference, opaque_iter::opaque_slice_iter},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

opaque_slice_iter! {
    /// An iterator over the [`ElementAttribute`]s in an [`Attrlist`], returned
    /// by [`Attrlist::attributes`].
    pub struct ElementAttributes<'a> yielding ElementAttribute<'a>;
}

/// The source text that’s used to define attributes for an element is referred
/// to as an **attrlist.** An attrlist is always enclosed in a pair of square
/// brackets. This applies for block attributes as well as attributes on a block
/// or inline macro. The processor splits the attrlist into individual attribute
/// entries, determines whether each entry is a positional or named attribute,
/// parses the entry accordingly, and assigns the result as an attribute on the
/// node.
#[derive(Clone, Eq, Hash, PartialEq, Default)]
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

        // 1-based counter over every comma-delimited entry, incremented per
        // entry – named attributes and blank (`nil`) slots included – so that
        // positional attributes are numbered the way Asciidoctor numbers them
        // (see `nth_attribute`).
        let mut entry_number = 0usize;

        let after_index = loop {
            entry_number += 1;

            let (mut attr, new_index, warning_types) = ElementAttribute::parse(
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
                warnings.push(Warning::new(source, warning_type));
            }

            // Shorthand items (the `#id`, `.role`, and `%option` entries) are
            // only recognized in the first attribute position. Once the first
            // attribute has been parsed – whether it was positional or named –
            // disable shorthand parsing so that, for example, a `%header`
            // entered after a named `cols` attribute is not mistaken for an
            // option (the processor ignores it).
            parse_shorthand_items = false;

            let mut after = Span::new(source_cow.as_ref()).discard(new_index);

            // A completely empty (or whitespace-only) attribute list: the first
            // entry is an empty, *unquoted* positional with nothing after it.
            // Yield no attributes. An explicit empty *quoted* positional
            // (`""` / `''`) carries a value and is kept below, so it is excluded
            // here by `!attr.value_is_quoted()`.
            if attr.name().is_none()
                && attr.value().is_empty()
                && !attr.value_is_quoted()
                && after.is_empty()
                && attributes.is_empty()
            {
                break index;
            }

            if attr.name().is_some() {
                // A named attribute whose value is the literal `None` unsets the
                // attribute (Asciidoctor semantics); it still consumes a
                // position but is not stored.
                if attr.value() != "None" {
                    attributes.push(attr);
                }
            } else if !attr.value().is_empty() || attr.value_is_quoted() {
                // A positional attribute – including an explicit empty quoted
                // value (`""` / `''`). Record its position so later positionals
                // stay aligned across named and blank entries.
                attr.set_positional_index(entry_number);
                attributes.push(attr);
            }

            // Otherwise this is an empty, unquoted positional: a blank (`nil`)
            // slot. It consumes `entry_number` (already incremented) but is not
            // stored, so a later positional keeps its Asciidoctor position.

            after = after.take_whitespace().after;

            match after.take_prefix(",") {
                Some(comma) => {
                    after = comma.after.take_whitespace().after;

                    if after.starts_with(',') {
                        warnings.push(Warning::new(source, WarningType::EmptyAttributeValue));

                        // Consume the blank slot between consecutive commas here,
                        // advancing the position counter past it.
                        entry_number += 1;
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
            warnings.push(Warning::new(
                source,
                WarningType::MissingCommaAfterQuotedAttributeValue,
            ));
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

    /// Build the attribute list implied by a language-aware fenced code block
    /// (`` ```lang ``).
    ///
    /// A fenced code block whose opening fence carries a language is shorthand
    /// for a source block: it is equivalent to a `[source,<language>]`
    /// attribute list applied to a listing block. The synthesized list
    /// therefore carries the `source` block style in the first position and
    /// the language in the second, so that downstream consumers resolve the
    /// block to a source (highlighted listing) block and can read the
    /// language via [`nth_attribute(2)`](Self::nth_attribute) – without
    /// this parser performing any syntax highlighting itself.
    ///
    /// The `source` span is set to the language span, since that is the only
    /// portion of the synthesized list that appears in the document source.
    pub(crate) fn source_with_language(language: Span<'src>) -> Self {
        Self {
            attributes: vec![
                ElementAttribute::synthesized_source_style(),
                ElementAttribute::positional_from_span(language),
            ],
            anchor: None,
            source: language,
        }
    }

    /// Merge a subsequent block attribute line into this one.
    ///
    /// A block can be preceded by more than one attribute list line (optionally
    /// straddling the block title). Asciidoctor merges every such line into a
    /// single set of attributes, where a later line wins on a name (or
    /// position) conflict and otherwise accumulates. This method folds `later`
    /// into `self` using those semantics:
    ///
    /// * **Named attributes** accumulate; a later attribute with the same name
    ///   replaces the earlier one (in place, preserving order).
    /// * **Positional attributes** are matched by position. A later positional
    ///   replaces the earlier one at the same position. The first positional
    ///   additionally carries the block style and shorthand items (`#id`,
    ///   `.role`, `%option`), which are merged via
    ///   [`ElementAttribute::merge_block_style_shorthand`].
    /// * **Roles** follow Asciidoctor's running model once a formal `role=`
    ///   entry is in play: a formal `role=` *replaces* every role accumulated
    ///   so far, while shorthand `.role` entries *append*. Because a replacing
    ///   `role=` on one line can sit between shorthand roles on earlier and
    ///   later lines, the resolved (ordered) role list is folded into the
    ///   formal `role` attribute and the first positional is left free of role
    ///   shorthand, so [`roles`](Self::roles) reports the resolved list. When
    ///   no formal `role=` is involved, shorthand roles simply accumulate as
    ///   above.
    /// * The **anchor** is taken from the later line if it specifies one.
    ///
    /// The `source` span is left pointing at the first line, since the merged
    /// attributes no longer correspond to a single contiguous span.
    pub(crate) fn merge_block_attribute_line(&mut self, later: Attrlist<'src>) {
        // Roles only need the running-model treatment once a formal `role=`
        // entry appears (on this line or an earlier, already-folded one).
        // Otherwise shorthand roles accumulate through the ordinary first-
        // positional merge below, unchanged.
        let fold_roles =
            self.named_attribute("role").is_some() || later.named_attribute("role").is_some();

        let resolved_roles: Option<Vec<String>> = if fold_roles {
            // The roles accumulated so far, in resolved order (a folded `role`
            // attribute holds them; failing that, `self`'s shorthand/formal
            // roles are read the ordinary way).
            let current: Vec<String> = self.roles().iter().map(|r| r.to_string()).collect();

            // A formal `role=` on the later line replaces the running list;
            // otherwise it carries forward. The later line's shorthand roles
            // then append (matching Asciidoctor, where a line's `role=` is set
            // before its shorthand roles are appended).
            let later_formal: Option<Vec<String>> = later
                .named_attribute("role")
                .map(|attr| split_role_value(attr.value()).map(str::to_string).collect());

            let later_shorthand: Vec<String> = later
                .nth_attribute(1)
                .map(|attr| attr.roles().iter().map(|r| r.to_string()).collect())
                .unwrap_or_default();

            let mut resolved = later_formal.unwrap_or(current);
            resolved.extend(later_shorthand);

            // Strip role shorthand from our own first positional so the formal
            // `role` attribute set below is the only remaining source of roles.
            if let Some(existing) = self.nth_positional_mut(1) {
                *existing = existing.without_shorthand_roles();
            }

            Some(resolved)
        } else {
            None
        };

        let Attrlist {
            attributes: later_attributes,
            anchor: later_anchor,
            source: _,
        } = later;

        if later_anchor.is_some() {
            self.anchor = later_anchor;
        }

        for attr in later_attributes {
            // When folding roles, the resolved role list is applied afterward,
            // so skip the later line's formal `role` attribute here (it is
            // accounted for in `resolved_roles`).
            if fold_roles && attr.name_str() == Some("role") {
                continue;
            }

            // An attribute carries a positional index exactly when it is a
            // positional (unnamed) attribute; a named attribute has `None`.
            // Dispatching on the index keeps a positional at its Asciidoctor
            // position – the same 1-based entry count `nth_attribute` uses,
            // which includes named entries and blank slots – so positions stay
            // aligned across lines even when a later line interleaves named
            // attributes before a positional.
            match attr.positional_index() {
                // Named: accumulate, with a later attribute replacing the
                // earlier one of the same name in place.
                None => {
                    if let Some(existing) = self
                        .attributes
                        .iter_mut()
                        .find(|a| a.name_str() == attr.name_str())
                    {
                        *existing = attr;
                    } else {
                        self.attributes.push(attr);
                    }
                }

                // The first positional additionally carries the block style and
                // shorthand items (`#id`, `.role`, `%option`), which are merged.
                // While folding roles, its role shorthand is dropped so roles
                // flow solely through the formal `role` attribute.
                Some(1) => {
                    let attr = if fold_roles {
                        attr.without_shorthand_roles()
                    } else {
                        attr
                    };

                    if let Some(existing) = self.nth_positional_mut(1) {
                        *existing = ElementAttribute::merge_block_style_shorthand(existing, &attr);
                    } else {
                        self.attributes.push(attr);
                    }
                }

                // A later positional replaces the earlier one at the same
                // position, otherwise it extends the list.
                Some(position) => {
                    if let Some(existing) = self.nth_positional_mut(position) {
                        *existing = attr;
                    } else {
                        self.attributes.push(attr);
                    }
                }
            }
        }

        // Record the resolved roles in the formal `role` attribute.
        if let Some(resolved) = resolved_roles {
            self.set_role_attribute(resolved);
        }
    }

    /// Replace (or clear) the formal `role` attribute with the given resolved
    /// role list. Called by
    /// [`merge_block_attribute_line`](Self::merge_block_attribute_line) once
    /// roles have been resolved under Asciidoctor's running model. An empty
    /// list removes any existing `role` attribute.
    fn set_role_attribute(&mut self, roles: Vec<String>) {
        if roles.is_empty() {
            self.attributes.retain(|a| a.name_str() != Some("role"));
            return;
        }

        let attr = ElementAttribute::synthesized_role(roles.join(" "));
        if let Some(existing) = self
            .attributes
            .iter_mut()
            .find(|a| a.name_str() == Some("role"))
        {
            *existing = attr;
        } else {
            self.attributes.push(attr);
        }
    }

    /// Return a mutable reference to the positional attribute at (1-based)
    /// Asciidoctor position `n` – the position recorded on each attribute, not
    /// its ordinal among stored positionals (the two differ once named entries
    /// or blank slots consume positions). See
    /// [`nth_attribute`](Self::nth_attribute).
    ///
    /// `n` must be 1 or greater; the only caller is
    /// [`merge_block_attribute_line`](Self::merge_block_attribute_line), which
    /// always passes a positive position.
    fn nth_positional_mut(&mut self, n: usize) -> Option<&mut ElementAttribute<'src>> {
        debug_assert!(n >= 1, "nth_positional_mut requires a 1-based position");

        self.attributes
            .iter_mut()
            .find(|attr| attr.positional_index() == Some(n))
    }

    /// Returns an iterator over the attributes contained within
    /// this attrlist.
    pub fn attributes(&'src self) -> ElementAttributes<'src> {
        ElementAttributes::new(&self.attributes)
    }

    /// Returns the anchor found in this attribute list, if any.
    pub fn anchor(&'src self) -> Option<&'src str> {
        self.anchor.as_deref()
    }

    /// Returns the `title=` attribute's value and whether the normal
    /// substitution group has already been applied to it, if the attribute is
    /// present.
    ///
    /// Unlike [`named_attribute`](Self::named_attribute), this borrows for the
    /// duration of the call only, so it can be read from an `Attrlist` that is
    /// about to be moved (as when block metadata derives a block title from a
    /// `title=` attribute before storing the attribute list on the block). The
    /// returned flag (see [`ElementAttribute::value_is_substituted`]) lets the
    /// caller avoid substituting an already-substituted (single-quoted) value a
    /// second time.
    pub(crate) fn title_attribute(&self) -> Option<(&str, bool)> {
        self.attributes
            .iter()
            .find(|attr| attr.name_str() == Some("title"))
            .map(|attr| (attr.value_str(), attr.value_is_substituted()))
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
    /// **IMPORTANT:** Positions are numbered the way Asciidoctor numbers them:
    /// every comma-delimited entry consumes a position, including named entries
    /// and blank (`nil`) slots. A later positional therefore keeps its position
    /// even when an earlier entry is named or left blank (e.g.
    /// `image::x[Alt,,3]` has `Alt` at position 1 and `3` at position 3,
    /// with position 2 empty). A position that is empty, or that is
    /// occupied by a named attribute, yields `None`.
    pub fn nth_attribute(&'src self, n: usize) -> Option<&'src ElementAttribute<'src>> {
        if n == 0 {
            None
        } else {
            self.attributes
                .iter()
                .find(|attr| attr.positional_index() == Some(n))
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
            roles.extend(split_role_value(role_attr.value()));
        }

        roles
    }

    /// Recovers the role from a quote-delimited first positional attribute (for
    /// example `['role']`) in a quoted-text attribute list.
    ///
    /// This mirrors the `else` branch of Asciidoctor's
    /// `parse_quoted_text_attributes`: when the first positional attribute is
    /// not shorthand (it does not begin with `.` or `#`), Asciidoctor treats
    /// the entire first positional – verbatim, quote characters included –
    /// as the role. The shorthand parser used for the general attribute
    /// list instead strips the surrounding quotes and records no role or
    /// block style for such a value, so a quoted role would otherwise be
    /// dropped.
    ///
    /// Returns the verbatim role only when the first positional attribute was
    /// genuinely quote-delimited; otherwise the normal shorthand path already
    /// produced the role, id, and block style, so this returns `None`.
    pub(crate) fn quoted_text_fallback_role(&'src self) -> Option<&'src str> {
        if !self.nth_attribute(1)?.value_is_quoted() {
            return None;
        }

        // Asciidoctor's `parse_quoted_text_attributes` considers only the first
        // positional attribute – the source up to the first comma – and uses it
        // verbatim (quote characters included) as the role. The comma split is on
        // the raw source, matching Asciidoctor's `str.slice 0, (str.index ',')`,
        // so a comma *inside* the quotes truncates the role there too (e.g.
        // `['a,b']` yields the role `'a`) rather than being treated as quoted
        // content. A quote-delimited first positional always leaves at least its
        // opening quote here, so the slice is never empty.
        let raw = self.source.data();
        Some(raw.split_once(',').map_or(raw, |(first, _)| first).trim())
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
            options.append(&mut split_options(option_attr.value()));
        }

        if let Some(option_attr) = self.named_attribute("options") {
            options.append(&mut split_options(option_attr.value()));
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

/// Split an `opts`/`options` attribute value into individual option tokens,
/// matching Asciidoctor: split on commas, trim surrounding whitespace from each
/// token, and drop empty tokens. So `'opt1,,opt2 , opt3'` yields `opt1`,
/// `opt2`, `opt3`.
fn split_options(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|opt| !opt.is_empty())
        .collect()
}

/// Split a formal `role` attribute value into its individual role names: split
/// on ASCII spaces and drop empty tokens. So `'role1  role2'` yields `role1`,
/// `role2`.
///
/// This is the single source of truth for how a `role=` value is tokenized,
/// shared by [`Attrlist::roles`] (which borrows the names) and the
/// block-attribute merge (which owns them), so the two can never diverge.
fn split_role_value(value: &str) -> impl Iterator<Item = &str> {
    value.split(' ').filter(|role| !role.is_empty())
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

        // A leading comma leaves position 1 blank (a `nil` slot, as in
        // Asciidoctor): it consumes the position but stores no attribute, so
        // `300` and `400` remain at positions 2 and 3.
        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
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

        // Position 1 is the blank slot: no attribute there.
        assert!(mi.item.nth_attribute(1).is_none());
        assert!(mi.item.named_or_positional_attribute("alt", 1).is_none());

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
                warning: WarningType::EmptyShorthandName,
            }]
        );
    }

    #[test]
    fn merge_block_attribute_line_anchor_later_wins() {
        let p = Parser::default();

        let mut first = crate::attributes::Attrlist::parse(
            crate::Span::new("[id1]"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        let later = crate::attributes::Attrlist::parse(
            crate::Span::new("[id2]"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        assert_eq!(first.anchor(), Some("id1"));
        first.merge_block_attribute_line(later);
        assert_eq!(first.anchor(), Some("id2"));
    }

    #[test]
    fn merge_block_attribute_line_anchor_retained_when_later_has_none() {
        let p = Parser::default();

        let mut first = crate::attributes::Attrlist::parse(
            crate::Span::new("[id1]"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        let later = crate::attributes::Attrlist::parse(
            crate::Span::new("foo=bar"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        first.merge_block_attribute_line(later);
        assert_eq!(first.anchor(), Some("id1"));
        assert_eq!(first.named_attribute("foo").unwrap().value(), "bar");
    }

    #[test]
    fn merge_block_attribute_line_positions_account_for_named_entries() {
        // A later line whose positional is preceded by a named attribute must
        // merge at its Asciidoctor position (which counts the named entry), not
        // at its ordinal among unnamed entries. Here `Author2` is the later
        // line's *second* entry, so it replaces position 2 (`Author1`) rather
        // than merging into position 1 (`quote`); `Extra` is position 3.
        let p = Parser::default();

        let mut first = crate::attributes::Attrlist::parse(
            crate::Span::new("quote,Author1"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        let later = crate::attributes::Attrlist::parse(
            crate::Span::new("width=300,Author2,Extra"),
            &p,
            AttrlistContext::Block,
        )
        .unwrap_if_no_warnings()
        .item;

        first.merge_block_attribute_line(later);

        assert_eq!(first.nth_attribute(1).unwrap().value(), "quote");
        assert_eq!(first.nth_attribute(2).unwrap().value(), "Author2");
        assert_eq!(first.nth_attribute(3).unwrap().value(), "Extra");
        assert_eq!(first.named_attribute("width").unwrap().value(), "300");
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

        #[test]
        fn quoted_first_positional_becomes_verbatim_role() {
            let p = Parser::default();

            // A quote-delimited first positional carries no shorthand role or
            // block style, so `quoted_text_fallback_role` recovers it verbatim
            // (quotes included), mirroring Asciidoctor's
            // `parse_quoted_text_attributes`.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'role'"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert!(mi.item.roles().is_empty());
            assert!(mi.item.block_style().is_none());
            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'role'");

            // Only the first positional (the source up to the first comma) is
            // considered, mirroring Asciidoctor's `parse_quoted_text_attributes`.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'role',keep=dropped"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'role'");

            // The comma boundary is applied to the raw source, matching
            // Asciidoctor's `str.slice 0, (str.index ',')`, so a comma inside the
            // quotes truncates the role there too rather than being kept as
            // quoted content.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'a,b'"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'a");

            // An unquoted positional is handled by the normal block-style path,
            // so there is no fallback role to recover.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("role"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert!(mi.item.quoted_text_fallback_role().is_none());

            // A named-only or otherwise position-1-less attribute list has no
            // first positional attribute, so there is nothing to recover.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("id=x"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert!(mi.item.quoted_text_fallback_role().is_none());
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
