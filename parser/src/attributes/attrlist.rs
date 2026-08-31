use crate::{
    HasSpan, Parser, Span,
    attributes::{
        ElementAttribute,
        element_attribute::{
            MASKED_PIECE_PLACEHOLDER, ParseShorthand, SplicedValueEscaping, restore_into,
        },
    },
    content::{Content, apply_attributes},
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

    /// The attrlist text this list was parsed from, kept only when `source`'s
    /// own bytes are *not* that text. There are two such cases, and both are
    /// about the same thing — the bytes the parse actually read:
    ///
    ///   - [`parse`](Self::parse) substituted attribute references into the
    ///     text before splitting it, so what it parsed is the *expanded* text
    ///     and `source` holds the author's `{name}` spelling.
    ///   - [`into_owned`](Self::into_owned) rebuilt the list from a temporary
    ///     and re-tagged it with a coarser source span, so `source` holds
    ///     whatever that span covers rather than the attrlist text at all.
    ///
    /// `None` for every list whose `source.data()` already *is* the text it
    /// was parsed from — the common case.
    ///
    /// Only [`source_text`](Self::source_text) reads it, for
    /// [`quoted_text_fallback_role`](Self::quoted_text_fallback_role) — the one
    /// accessor that reads the attrlist's own text rather than a parsed
    /// attribute.
    source_text: Option<CowStr<'src>>,
}

impl<'src> Attrlist<'src> {
    /// Rebuilds this attribute list with every borrowed string copied into an
    /// owned one and `source` as its new span, so it can outlive the text it
    /// was parsed from.
    ///
    /// [`parse`](Self::parse) reads its `source` span's bytes **as content**,
    /// so an attribute list can only be parsed from a span that actually holds
    /// the attrlist text. A caller that has those bytes only in a temporary
    /// string — the inline AST builder, whose match string carries an escaped
    /// or attribute-expanded value that no `'src` slice reproduces — parses
    /// from a [`Span::new`] over that temporary and calls this to keep the
    /// result, passing the coarser source span the text corresponds to as the
    /// list's own location tag. Every parsed field is a [`CowStr`], so nothing
    /// but the location depends on the original span — with one exception this
    /// method itself handles:
    /// [`quoted_text_fallback_role`](Self::quoted_text_fallback_role) reads the
    /// attrlist's own *text*, which after the re-tag is no longer what `source`
    /// holds (the temporary's escaped or expanded bytes are precisely what no
    /// `'src` slice reproduces). An owned copy of the text this list was parsed
    /// from therefore rides along in
    /// [`source_text`](Self::source_text), so that accessor goes on reading it.
    ///
    /// That copy is taken through [`source_text`](Self::source_text) rather
    /// than off `self.source` directly, so a list whose own
    /// [`parse`](Self::parse) already expanded an attribute reference carries
    /// the *expanded* text forward rather than reinstating the `{name}`
    /// spelling the re-tag is discarding the span for.
    pub(crate) fn into_owned<'dst>(self, source: Span<'dst>) -> Attrlist<'dst> {
        let source_text = CowStr::from(self.source_text().to_string());

        Attrlist {
            attributes: self
                .attributes
                .into_iter()
                .map(ElementAttribute::into_owned)
                .collect(),
            anchor: self.anchor.map(CowStr::into_owned),
            source,
            source_text: Some(source_text),
        }
    }

    /// [`into_owned`](Self::into_owned), first substituting each
    /// [`MASKED_PIECE_PLACEHOLDER`] occurrence in every parsed value — and in
    /// the retained [`source_text`](Self::source_text) — with the next of
    /// `bodies`, in left-to-right document order.
    ///
    /// This is how a caller restores a masked construct that its attribute
    /// list text still holds. The restore has to happen **after** the parse,
    /// not before it: [`Attrlist::parse`] has to see the placeholder still in
    /// place, so the body's own bytes never reach the split that divides the
    /// list into entries, names, and values.
    ///
    /// A single shared cursor, starting at `0`, walks
    /// [`self.attributes`](Self) in the same order [`Attrlist::parse`] found
    /// them in — which is left-to-right document order, the same order a
    /// placeholder's own position in `bodies` was assigned in — so no index
    /// needs to be carried in the placeholder's own bytes. `source_text` is
    /// an independent full copy of the same text and is restored with a
    /// fresh cursor of its own, from `0` again, for the same reason. See
    /// [`ElementAttribute::into_owned_restoring`] for the per-attribute half,
    /// including what it does to the shorthand offsets, and
    /// [`nth_attribute_token_offset`](Self::nth_attribute_token_offset) /
    /// [`named_attribute_token_offset`](Self::named_attribute_token_offset)
    /// for a caller that needs to restore one attribute's value against a
    /// *slice* of a global body/node list instead of the whole list this
    /// method walks.
    pub(crate) fn into_owned_restoring<'dst>(
        self,
        source: Span<'dst>,
        bodies: &[&str],
    ) -> Attrlist<'dst> {
        let source_text = CowStr::from(self.source_text().to_string());
        let mut cursor = 0usize;

        let attributes = self
            .attributes
            .into_iter()
            .map(|attribute| attribute.into_owned_restoring(bodies, &mut cursor))
            .collect();

        Attrlist {
            attributes,
            // The anchor takes the plain conversion rather than a restoring
            // one. An anchor is only ever set for the `[…]`-delimited whole
            // -bracket form, and no restoring caller can present one: each
            // parses a macro's bracket capture, whose own pattern ends the
            // match at the first `]`, so `[x]` never arrives here intact.
            anchor: self.anchor.map(CowStr::into_owned),
            source,
            source_text: Some(restore_into(
                source_text,
                bodies,
                &mut 0usize,
                &mut [],
                &mut Vec::new(),
            )),
        }
    }

    /// The number of [`MASKED_PIECE_PLACEHOLDER`] occurrences in every
    /// attribute's name and value that come before `self.attributes[index]`,
    /// in this list's own parse order.
    ///
    /// A caller restoring one attribute's own value against a **slice** of a
    /// global body/node list — rather than the whole list
    /// [`into_owned_restoring`](Self::into_owned_restoring) walks — uses this
    /// to find where that attribute's own occurrences begin: the same
    /// position `into_owned_restoring`'s own shared cursor would be at once
    /// it reached this attribute. Counting occurrences needs no restore of
    /// its own — it reads the still-tokened `name`/`value` text directly — so
    /// this works before any restore has run.
    fn token_offset_before(&self, index: usize) -> usize {
        self.attributes
            .get(..index)
            .unwrap_or_default()
            .iter()
            .map(|attr| {
                attr.name()
                    .map_or(0, |name| name.matches(MASKED_PIECE_PLACEHOLDER).count())
                    + attr.value().matches(MASKED_PIECE_PLACEHOLDER).count()
            })
            .sum()
    }

    /// [`nth_attribute`](Self::nth_attribute), plus the offset into a global
    /// body/node list where that attribute's own placeholder occurrences
    /// begin (see [`token_offset_before`](Self::token_offset_before)).
    pub(crate) fn nth_attribute_token_offset(&self, n: usize) -> Option<usize> {
        let index = self
            .attributes
            .iter()
            .position(|attr| attr.positional_index() == Some(n))?;

        Some(self.token_offset_before(index))
    }

    /// [`named_attribute`](Self::named_attribute), plus the offset into a
    /// global body/node list where that attribute's own placeholder
    /// occurrences begin (see
    /// [`token_offset_before`](Self::token_offset_before)).
    pub(crate) fn named_attribute_token_offset(&self, name: &str) -> Option<usize> {
        let index = self
            .attributes
            .iter()
            .position(|attr| attr.name() == Some(name))?;

        Some(self.token_offset_before(index))
    }

    /// **IMPORTANT:** This `source` span passed to this function should NOT
    /// include the opening or closing square brackets for the attrlist.
    /// This is because the rules for closing brackets differ when parsing
    /// inline, macro, and block elements.
    pub(crate) fn parse(
        source: Span<'src>,
        parser: &Parser,
        attrlist_context: AttrlistContext,
    ) -> MatchAndWarnings<'src, MatchedItem<'src, Self>> {
        Self::parse_escaping(
            source,
            parser,
            attrlist_context,
            SplicedValueEscaping::Verbatim,
        )
    }

    /// [`parse`](Self::parse) over **tokened** macro-bracket text — what
    /// `tokened_bracket`/`tokened_text` (the macros step) hand back, in which
    /// each masked passthrough or STEM piece stands as one
    /// [`MASKED_PIECE_PLACEHOLDER`] occurrence and every other byte is escaped
    /// ([`escape_masked_piece_bytes`]).
    ///
    /// The difference is the attribute-reference substitution this parse runs
    /// of its own, below. That substitution is the **second** point at which
    /// bytes enter a tokened text — a `subs=` list naming `macros` without
    /// `attributes` reaches the macros step with every reference still
    /// unresolved, so it is this parse, not the content-level step, that
    /// expands one — and it runs *after* the tokener escaped its copy. Under
    /// [`SplicedValueEscaping::MaskedPieceBytes`] each resolved value is
    /// escaped as it is spliced, which is what keeps the tokener's property
    /// total: every reserved codepoint standing in the text this splits is one
    /// the tokener wrote, so [`restore_into`]'s positional walk has nothing to
    /// be fooled by and a document attribute's own reserved bytes come back
    /// out as the document wrote them.
    ///
    /// Every other caller wants [`parse`](Self::parse): ordinary content
    /// carries no such invariant, and escaping into it would put the escape's
    /// own bytes in front of a reader that never unescapes.
    ///
    /// [`escape_masked_piece_bytes`]: crate::attributes::element_attribute::escape_masked_piece_bytes
    pub(crate) fn parse_tokened(
        source: Span<'src>,
        parser: &Parser,
    ) -> MatchAndWarnings<'src, MatchedItem<'src, Self>> {
        Self::parse_escaping(
            source,
            parser,
            AttrlistContext::Inline,
            SplicedValueEscaping::MaskedPieceBytes,
        )
    }

    fn parse_escaping(
        source: Span<'src>,
        parser: &Parser,
        attrlist_context: AttrlistContext,
        escaping: SplicedValueEscaping,
    ) -> MatchAndWarnings<'src, MatchedItem<'src, Self>> {
        let mut attributes: Vec<ElementAttribute> = vec![];
        let mut parse_shorthand_items = true;
        let mut warnings: Vec<Warning<'src>> = vec![];

        // Apply attribute value substitutions before parsing attrlist content.
        let source_cow = if source.contains('{') && source.contains('}') {
            let mut content = Content::from(source);
            apply_attributes(&mut content, parser, escaping);
            CowStr::from(content.rendered.to_string())
        } else {
            CowStr::from(source.data())
        };

        // Every *parsed* field below comes out of `source_cow`, so an attribute
        // reference in a value is already expanded by the time a caller reads
        // it. `quoted_text_fallback_role` is the one accessor that reads the
        // list's own text instead, and it must see the same expanded bytes —
        // Asciidoctor's `parse_quoted_text_attributes` runs `sub_attributes`
        // over the list and *then* takes the first positional verbatim. So the
        // expanded text is retained whenever the substitution changed anything,
        // and `source.data()` goes on serving the (overwhelmingly common) case
        // where it did not.
        let substituted = source_cow.as_ref() != source.data();

        if source_cow.starts_with('[') && source_cow.ends_with(']') {
            let anchor = source_cow[1..source_cow.len() - 1].to_owned();

            return MatchAndWarnings {
                item: MatchedItem {
                    item: Self {
                        attributes,
                        anchor: Some(CowStr::from(anchor)),
                        source,
                        source_text: substituted.then_some(source_cow),
                    },
                    after: source.discard_all(),
                },
                warnings,
            };
        }

        let mut index = 0;

        // 1-based counter over every comma-delimited entry, incremented per
        // entry — named attributes and blank (`nil`) slots included — so that
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

            // Because we do attribute value substitution early on in parsing,
            // we can't pinpoint the exact location of warnings in
            // an attribute list. For that reason, individual
            // attribute parsing only returns the warning type and we
            // then map it back to the entire attrlist source.
            for warning_type in warning_types {
                warnings.push(Warning::new(source, warning_type));
            }

            // Shorthand items (the `#id`, `.role`, and `%option` entries) are
            // only recognized in the first attribute position. Once the first
            // attribute has been parsed — whether it was positional or named —
            // disable shorthand parsing so that, for example, a `%header`
            // entered after a named `cols` attribute is not mistaken for an
            // option (the processor ignores it).
            parse_shorthand_items = false;

            let mut after = Span::new(source_cow.as_ref()).discard(new_index);

            // A completely empty (or whitespace-only) attribute list: the first
            // entry is an empty, *unquoted* positional with nothing after it.
            // Yield no attributes. An explicit empty *quoted* positional
            // (`""` / `''`) carries a value and is kept below, so it is
            // excluded here by `!attr.value_is_quoted()`.
            if attr.name().is_none()
                && attr.value().is_empty()
                && !attr.value_is_quoted()
                && after.is_empty()
                && attributes.is_empty()
            {
                break index;
            }

            if attr.name().is_some() {
                // A named attribute whose value is the literal `None` unsets
                // the attribute (Asciidoctor semantics); it
                // still consumes a position but is not stored.
                if attr.value() != "None" {
                    attributes.push(attr);
                }
            } else if !attr.value().is_empty() || attr.value_is_quoted() {
                // A positional attribute — including an explicit empty quoted
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

                        // Consume the blank slot between consecutive commas
                        // here, advancing the position
                        // counter past it.
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
                    source_text: substituted.then_some(source_cow),
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
    /// language via [`nth_attribute(2)`](Self::nth_attribute) — without
    /// this parser performing any syntax highlighting itself.
    ///
    /// An attribute list with no attributes, located at `source`.
    ///
    /// This is what "the node carried no attribute list" looks like. The
    /// inline nodes that can carry one — [`Image`](crate::inlines::Image),
    /// [`Styled`](crate::inlines::Styled) and [`Ref`](crate::inlines::Ref) —
    /// hold an `Attrlist` outright rather than an `Option<Attrlist>`, so every
    /// consumer reads attributes the same way whether the author wrote a list
    /// or not; the ones written without a list get this. `source` should be a
    /// zero-length slice of the node's own location, so the empty list's
    /// lifetime and position match the node it belongs to.
    ///
    /// It is public because those node fields are: a caller building a node by
    /// hand needs to be able to say "no attributes", and every other route to
    /// an `Attrlist` goes through parsing — which would cost an
    /// attribute-reference substitution pass and, more to the point, require a
    /// [`Parser`] where only a `Span` is at hand.
    #[must_use]
    pub fn empty(source: Span<'src>) -> Self {
        Self {
            attributes: vec![],
            anchor: None,
            source,
            source_text: None,
        }
    }

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

            // A synthesized list has no attrlist text in the document at all
            // (`source` is the language span), and nothing reads this for it.
            source_text: None,
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

            // Kept from `self`, like `source` itself: only inline quoted text
            // reads the list's own text, and an inline list is never merged.
            source_text: _,
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
            // position — the same 1-based entry count `nth_attribute` uses,
            // which includes named entries and blank slots — so positions stay
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
    /// Asciidoctor position `n` — the position recorded on each attribute, not
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

    /// [`roles`](Self::roles), pairing each role with the offset into a
    /// global body/node list where *its own* placeholder occurrences begin
    /// (see [`token_offset_before`](Self::token_offset_before)) — a role
    /// from the first positional attribute's own shorthand items and one
    /// from a named `role=` attribute are two different attributes in this
    /// list's own parse order, so a caller restoring a role read off a
    /// still-tokened list (e.g. `untranslated_value` in the macros step)
    /// cannot use one starting offset for both.
    ///
    /// Nor can two roles split out of the *same* source attribute
    /// (`role=++a++ ++b++` is one attribute, two space-separated roles, each
    /// with its own placeholder): each role after the first has to skip past
    /// every placeholder occurrence the roles *before* it, in the same
    /// value, already account for — so the offset is a running count, seeded
    /// from the source attribute's own base offset and advanced by each
    /// role's own occurrence count as the split walks left to right, not one
    /// shared starting point per attribute.
    pub(crate) fn roles_with_token_offset(&'src self) -> Vec<(&'src str, usize)> {
        let mut roles: Vec<(&'src str, usize)> = vec![];

        if let Some(base) = self.nth_attribute_token_offset(1)
            && let Some(attr1) = self.nth_attribute(1)
        {
            let mut offset = base;

            for role in attr1.roles() {
                roles.push((role, offset));
                offset += role.matches(MASKED_PIECE_PLACEHOLDER).count();
            }
        }

        if let Some(base) = self.named_attribute_token_offset("role")
            && let Some(role_attr) = self.named_attribute("role")
        {
            let mut offset = base;

            for role in split_role_value(role_attr.value()) {
                roles.push((role, offset));
                offset += role.matches(MASKED_PIECE_PLACEHOLDER).count();
            }
        }

        roles
    }

    /// The attrlist text this list was parsed from.
    ///
    /// For every list parsed straight from the source it describes, that is
    /// its [`source`](Self::span) span's own bytes. For one rebuilt by
    /// [`into_owned`](Self::into_owned) from a temporary — whose `source` is a
    /// coarser *location tag* rather than the text (design §4.4) — it is the
    /// owned copy that method kept.
    fn source_text(&'src self) -> &'src str {
        match &self.source_text {
            Some(text) => text.as_ref(),
            None => self.source.data(),
        }
    }

    /// Recovers the role from a quote-delimited first positional attribute (for
    /// example `['role']`) in a quoted-text attribute list.
    ///
    /// This mirrors the `else` branch of Asciidoctor's
    /// `parse_quoted_text_attributes`: when the first positional attribute is
    /// not shorthand (it does not begin with `.` or `#`), Asciidoctor treats
    /// the entire first positional — verbatim, quote characters included —
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
        // positional attribute — the source up to the first comma — and uses it
        // verbatim (quote characters included) as the role. The comma split is
        // on the raw source, matching Asciidoctor's `str.slice 0,
        // (str.index ',')`, so a comma *inside* the quotes truncates
        // the role there too (e.g. `['a,b']` yields the role `'a`)
        // rather than being treated as quoted content. A
        // quote-delimited first positional always leaves at least its
        // opening quote here, so the slice is never empty.
        let raw = self.source_text();
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
        // PERF: Might help to optimize away the construction of the options
        // Vec.
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
    fn attribute_reference_substitution_shifts_a_placeholder_byte_offset_in_source_text() {
        // Direct evidence for the first blocker that ruled out carrying a
        // byte-offset table through `Attrlist::parse`, checked against this
        // method's own `source_text()`
        // rather than only a full document's final rendered HTML (as
        // `tests/sentinels.rs`'s
        // `an_attrlist_level_reference_expansion_moves_a_placeholder_in_the_tokened_text`
        // does) — so a future change to how this substitution works cannot
        // silently stop supporting the claim while rendering still happens to
        // come out right.
        //
        // `tokened_bracket`/`tokened_text` would write
        // `MASKED_PIECE_PLACEHOLDER` at byte 17 of
        // `alt={name},title=<placeholder>` — right after `title=`. This
        // method's own attribute-reference substitution
        // expands `{name}` before splitting entries — unconditional whenever
        // the text holds both a `{` and a `}` — so the placeholder the parsed
        // entries actually see has moved by however much longer the expanded
        // `alt` value is than the reference that named it.
        use crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER;

        let p = Parser::default().with_intrinsic_attribute(
            "name",
            "a-much-longer-value",
            ModificationContext::Anywhere,
        );

        let source = format!("alt={{name}},title={MASKED_PIECE_PLACEHOLDER}");
        let written_offset = source.find(MASKED_PIECE_PLACEHOLDER).unwrap();
        assert_eq!(written_offset, 17);

        let attrlist = crate::attributes::Attrlist::parse(
            crate::Span::new(&source),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings()
        .item;

        let split_offset = attrlist
            .source_text()
            .find(MASKED_PIECE_PLACEHOLDER)
            .unwrap();

        assert_eq!(
            split_offset,
            30,
            "the substitution's own extra length must have moved the placeholder: {:?}",
            attrlist.source_text()
        );
    }

    #[test]
    fn a_restore_copies_an_unpaired_escape_introducer_through() {
        // The restore walk's decode half is **total** over arbitrary input:
        // an escape introducer followed by a byte that is not one of the
        // three tags this crate writes is copied through with the bytes after
        // it, rather than eating one as a tag (see `escaped_literal`), and
        // the genuine placeholder further along still restores.
        //
        // Both points at which bytes enter a tokened text escape now — the
        // tokener's own copy, and the splice inside `Attrlist::parse_tokened`
        // — so the pipeline itself no longer produces an unpaired introducer.
        // That is exactly why this is pinned here, over a hand-written
        // tokened text, rather than through a document: totality is what lets
        // every consumer run the unescape unconditionally, and it should not
        // quietly stop holding just because nothing upstream exercises it.
        use crate::attributes::element_attribute::{MASKED_PIECE_ESCAPE, MASKED_PIECE_PLACEHOLDER};

        let p = Parser::default();
        let source = format!("alt=p{MASKED_PIECE_ESCAPE}zq,title={MASKED_PIECE_PLACEHOLDER}");

        let attrlist = crate::attributes::Attrlist::parse_tokened(crate::Span::new(&source), &p)
            .unwrap_if_no_warnings()
            .item
            .into_owned_restoring(crate::Span::new("whatever"), &["real"]);

        assert_eq!(
            attrlist.named_attribute("alt").map(|attr| attr.value()),
            Some(format!("p{MASKED_PIECE_ESCAPE}zq").as_str())
        );

        assert_eq!(
            attrlist.named_attribute("title").map(|attr| attr.value()),
            Some("real")
        );
    }

    #[test]
    fn a_restore_copies_an_occurrence_past_the_end_of_bodies_through() {
        // The restore walk fails **closed** when it finds more placeholder
        // occurrences than the caller supplied bodies for: the surplus
        // occurrence is copied through verbatim rather than panicking or
        // taking a later occurrence's body, and every occurrence the caller
        // *did* supply a body for still restores.
        //
        // No caller can produce this — each tokens exactly as many pieces as
        // it supplies bodies for, and since the tokened parse's own
        // attribute-reference splice is escaped
        // (`SplicedValueEscaping::MaskedPieceBytes`) an expansion can no
        // longer add an occurrence behind the tokener's back either, which is
        // what used to reach this arm. It is pinned directly instead, so a
        // caller that someday breaks the count is caught by this contract
        // rather than by a corrupted neighbouring restore.
        use crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER;

        let p = Parser::default();
        let source = format!("alt={MASKED_PIECE_PLACEHOLDER},title={MASKED_PIECE_PLACEHOLDER}");

        // Two occurrences, one body.
        let attrlist = crate::attributes::Attrlist::parse_tokened(crate::Span::new(&source), &p)
            .unwrap_if_no_warnings()
            .item
            .into_owned_restoring(crate::Span::new("whatever"), &["real"]);

        assert_eq!(
            attrlist.named_attribute("alt").map(|attr| attr.value()),
            Some("real")
        );

        assert_eq!(
            attrlist.named_attribute("title").map(|attr| attr.value()),
            Some(MASKED_PIECE_PLACEHOLDER)
        );
    }

    #[test]
    fn token_offset_helpers_count_placeholders_before_the_target_attribute() {
        // Two masked pieces (each one `MASKED_PIECE_PLACEHOLDER` occurrence,
        // as `tokened_bracket`/`tokened_text` would leave it before a
        // restore): one inside a named `role=` attribute, first in this
        // list's own parse order, and one inside the third (positional)
        // entry. A caller restoring the third entry's own value against a
        // *slice* of a global body/node list has to start past the role's
        // own occurrence, not at the whole list's first one — this is the
        // scenario the two `_token_offset` accessors exist for.
        use crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER;

        let source = format!("role={MASKED_PIECE_PLACEHOLDER},x,{MASKED_PIECE_PLACEHOLDER}");
        let p = Parser::default();
        let attrlist = crate::attributes::Attrlist::parse(
            crate::Span::new(&source),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings()
        .item;

        // Nothing precedes `role=` itself.
        assert_eq!(attrlist.named_attribute_token_offset("role"), Some(0));

        // The third entry (positional index 3) is the *third* attribute in
        // vec order (role, then the blank `x`... no, `x` is itself
        // positional index 2) — one placeholder (role's own) precedes it.
        assert_eq!(attrlist.nth_attribute_token_offset(3), Some(1));

        // A name with no matching attribute finds nothing to offset.
        assert_eq!(attrlist.named_attribute_token_offset("nope"), None);
        assert_eq!(attrlist.nth_attribute_token_offset(99), None);
    }

    #[test]
    fn roles_with_token_offset_pairs_each_role_with_its_own_source_attributes_offset() {
        // A role from the first positional's own shorthand items and one
        // from a named `role=` attribute are two different attributes, and
        // `roles()` merges them into one flat list with no way back to
        // either source — which is exactly why a restoring caller
        // (`untranslated_value`) needs the offset alongside each role rather
        // than one shared starting point.
        use crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER;

        let source =
            format!("{MASKED_PIECE_PLACEHOLDER}.shorthand,role={MASKED_PIECE_PLACEHOLDER}");
        let p = Parser::default();
        let attrlist = crate::attributes::Attrlist::parse(
            crate::Span::new(&source),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings()
        .item;

        let roles = attrlist.roles_with_token_offset();

        assert_eq!(
            roles,
            vec![("shorthand", 0), (MASKED_PIECE_PLACEHOLDER, 1)],
            "the shorthand role's own attribute is first, so nothing precedes it \
             (offset 0); the named `role=` attribute — still unrestored, so its \
             value is the placeholder itself — is the second attribute, past the \
             shorthand's own one placeholder occurrence (offset 1)"
        );
    }

    #[test]
    fn roles_with_token_offset_advances_past_each_earlier_role_in_the_same_attribute() {
        // Two space-separated roles inside the *same* `role=` attribute
        // (`role=++a++ ++b++`), each carrying its own placeholder — a single
        // attribute, not two, so `named_attribute_token_offset` gives both
        // roles the same *base*, and the second role's own offset has to
        // additionally skip past the first role's own one occurrence, not
        // reuse the base as if only one role were there (Greptile
        // https://github.com/asciidoc-rs/asciidoc-parser/pull/1349#discussion_r3890749214).
        use crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER;

        let source = format!("role={MASKED_PIECE_PLACEHOLDER} {MASKED_PIECE_PLACEHOLDER}");
        let p = Parser::default();
        let attrlist = crate::attributes::Attrlist::parse(
            crate::Span::new(&source),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings()
        .item;

        assert_eq!(
            attrlist.roles_with_token_offset(),
            vec![(MASKED_PIECE_PLACEHOLDER, 0), (MASKED_PIECE_PLACEHOLDER, 1),],
            "the first role's own occurrence is offset 0; the second role's own \
             occurrence must skip past it rather than reusing offset 0"
        );
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
    fn merge_block_attribute_line_with_empty_shorthand_names() {
        // Two consecutive block attribute lines whose shorthand contains runs
        // of delimiters used to abort a debug build: merging re-parsed the
        // synthesized shorthand, and the empty option names it re-encoded
        // raised `EmptyShorthandName` against a `debug_assert!` that claimed
        // the situation could not arise. The document now parses the same way
        // in both profiles, with the malformed shorthand reported once — as an
        // ordinary warning against the line that actually carried it.
        // See https://github.com/asciidoc-rs/asciidoc-parser/issues/1237.
        let mut p = Parser::default();
        let doc = p.parse("\\i\n[%%%%\t\t%%%f]\r\n[f]");

        let empty_shorthand_name_warnings = doc
            .warnings()
            .filter(|w| w.warning == WarningType::EmptyShorthandName)
            .count();

        // Six delimiters name nothing: the four leading `%`, the `%` followed
        // only by tabs, and the two `%` after them. (The whitespace-only name
        // was silently accepted until
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/1273.)
        assert_eq!(empty_shorthand_name_warnings, 6);

        assert!(doc.warnings().all(|w| w.source.data() == "%%%%\t\t%%%f"
            || w.warning == WarningType::MissingBlockAfterTitleOrAttributeList));
    }

    #[test]
    fn whitespace_only_shorthand_id_does_not_shadow_a_real_one() {
        // A shorthand name made only of whitespace was accepted without a
        // warning and then surfaced as an empty ID. Because `id()` reports the
        // first shorthand item that starts with `#`, that empty ID hid a real
        // one declared later in the same attrlist.
        // See https://github.com/asciidoc-rs/asciidoc-parser/issues/1273.
        let mut p = Parser::default();
        let doc = p.parse("[x#\t#realid]\nhello");

        assert_eq!(doc.child_blocks().next().unwrap().id().unwrap(), "realid");

        assert_eq!(
            doc.warnings()
                .filter(|w| w.warning == WarningType::EmptyShorthandName)
                .count(),
            1
        );
    }

    #[test]
    fn whitespace_only_shorthand_ids_do_not_collide_in_the_catalog() {
        // Each whitespace-only ID used to be registered in the ID catalog as
        // the empty string, so a second one reported `DuplicateId("")`.
        // See https://github.com/asciidoc-rs/asciidoc-parser/issues/1273.
        let mut p = Parser::default();
        let doc = p.parse("[x#\t]\nhello\n\n[y#\t]\nworld");

        assert!(doc.child_blocks().all(|b| b.id().is_none()));

        assert!(
            !doc.warnings()
                .any(|w| matches!(w.warning, WarningType::DuplicateId(_)))
        );

        assert_eq!(
            doc.warnings()
                .filter(|w| w.warning == WarningType::EmptyShorthandName)
                .count(),
            2
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
            // considered, mirroring Asciidoctor's
            // `parse_quoted_text_attributes`.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'role',keep=dropped"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'role'");

            // The comma boundary is applied to the raw source, matching
            // Asciidoctor's `str.slice 0, (str.index ',')`, so a comma inside
            // the quotes truncates the role there too rather than
            // being kept as quoted content.
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

        #[test]
        fn an_owned_list_keeps_the_text_it_was_parsed_from() {
            let p = Parser::default();

            // `into_owned` re-tags a list parsed from a temporary with a
            // *coarser* source span — the inline AST builder's case, where the
            // attrlist text is the escaped or attribute-expanded bytes of a
            // match string and the span is only a location tag (design §4.4).
            // This is the one accessor that reads the list's own text rather
            // than a parsed attribute, so it reads the kept copy: recovering
            // the raw span's `'a<b'` here would drop the escaping the string
            // pipeline's own rendered `class` carries.
            let escaped = "'a&lt;b'";

            let owned = crate::attributes::Attrlist::parse(
                crate::Span::new(escaped),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings()
            .item
            .into_owned(crate::Span::new("'a<b'"));

            assert_eq!(owned.quoted_text_fallback_role().unwrap(), escaped);

            // The location tag is the span it was re-tagged with, unchanged.
            assert_eq!(owned.span().data(), "'a<b'");
        }

        #[test]
        fn quoted_first_positional_reads_the_substituted_text() {
            // `parse` expands attribute references over the whole list before
            // splitting it, so every *parsed* field is already expanded. The
            // verbatim role must come from those same expanded bytes:
            // Asciidoctor's `parse_quoted_text_attributes` runs
            // `sub_attributes` over the list and *then* takes the first
            // positional verbatim, and it does so regardless of the enclosing
            // block's `subs` list.
            let p = crate::Parser::default().with_intrinsic_attribute(
                "myrole",
                "highlight",
                crate::parser::ModificationContext::Anywhere,
            );

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'{myrole}'"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'highlight'");

            // The comma boundary is applied *after* the substitution, so an
            // expansion that introduces one truncates the role there too.
            let p = crate::Parser::default().with_intrinsic_attribute(
                "commarole",
                "a,b",
                crate::parser::ModificationContext::Anywhere,
            );

            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'{commarole}'"),
                &p,
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'a");

            // A missing attribute is left alone under the default
            // `attribute-missing=skip`, so the substitution is a no-op and the
            // source's own bytes still serve.
            let mi = crate::attributes::Attrlist::parse(
                crate::Span::new("'{missing}'"),
                &crate::Parser::default(),
                AttrlistContext::Inline,
            )
            .unwrap_if_no_warnings();

            assert_eq!(mi.item.quoted_text_fallback_role().unwrap(), "'{missing}'");
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
