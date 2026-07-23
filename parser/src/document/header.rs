use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, SubstitutionGroup},
    document::{
        Attribute, Author, AuthorLine, InterpretedValue, RevisionLine, matches_author_pattern,
    },
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// An AsciiDoc document may begin with a document header. The document header
/// encapsulates the document title, author and revision information,
/// document-wide attributes, and other document metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct Header<'src> {
    title_source: Option<Span<'src>>,
    title: Option<String>,
    doctitle: Option<String>,
    main_title: Option<String>,
    subtitle: Option<String>,
    id: Option<String>,
    roles: Vec<String>,
    attributes: Vec<Attribute<'src>>,
    author_line: Option<AuthorLine<'src>>,
    authors: Vec<Author>,
    revision_line: Option<RevisionLine<'src>>,
    comments: Vec<Span<'src>>,
    source: Span<'src>,
}

impl<'src> Header<'src> {
    pub(crate) fn parse(
        mut source: Span<'src>,
        parser: &mut Parser,
    ) -> MatchAndWarnings<'src, MatchedItem<'src, Self>> {
        let original_source = source.discard_empty_lines();

        let mut title_source: Option<Span<'src>> = None;
        let mut title: Option<String> = None;

        // State that mirrors Asciidoctor's `parse_document_header` doctitle
        // handling: whether an implicit `= Title` line was seen, the eager
        // (at-title-line) substitution stored in the `doctitle` attribute, and
        // whether a `:doctitle:` attribute entry appeared below the title (a
        // candidate to override the section title).
        let mut saw_implicit_title = false;
        let mut implicit_overridden_from_above = false;
        let mut implicit_doctitle_str: Option<String> = None;
        let mut doctitle_entry_after_title = false;

        let mut id: Option<String> = None;
        let mut roles: Vec<String> = vec![];
        let mut attributes: Vec<Attribute> = vec![];
        let mut author_line: Option<AuthorLine<'src>> = None;
        let mut author_attribute: Option<Author> = None;
        let mut authorinitials_from_entry = false;
        let mut revision_line: Option<RevisionLine<'src>> = None;
        let mut comments: Vec<Span<'src>> = vec![];
        let mut warnings: Vec<Warning<'src>> = vec![];

        // Aside from the title line, items can appear in almost any order.
        while !source.is_empty() {
            let line_mi = source.take_normalized_line();
            let line = line_mi.item;

            // A blank line after the title ends the header.
            if line.is_empty() {
                if title.is_some() {
                    break;
                }
                source = line_mi.after;
            } else if line.starts_with("//") && !line.starts_with("///") {
                comments.push(line);
                source = line_mi.after;
            } else if title.is_some()
                && let Some((after, terminated)) = skip_block_comment(line, line_mi.after)
            {
                // Once a title has been seen, a `////` block comment delimiter
                // opens a comment block within the header. Skip every line
                // through the matching closing delimiter (or to the end of the
                // input if the block is never closed), retaining the whole block
                // as a single comment so it is not mistaken for the author or
                // revision line. Blank lines inside the block do not terminate
                // the header.
                //
                // Before a title is seen there is no header author/revision
                // context to protect, so a leading `////` is left for the block
                // parser, which retains it as a body-level comment block.
                comments.push(source.trim_remainder(after).trim_trailing_line_end());

                // An unterminated comment block swallows the rest of the header
                // (any following attribute entries are never applied), so warn
                // as Asciidoctor does, anchoring the warning at the opening
                // delimiter. This mirrors the body-level comment block path,
                // which reports the same `UnterminatedDelimitedBlock` warning.
                if !terminated {
                    warnings.push(Warning {
                        source: line,
                        warning: WarningType::UnterminatedDelimitedBlock,
                        origin: None,
                    });
                }

                source = after;
            } else if line.starts_with(':')
                && let Some(attr) = Attribute::parse(source, parser)
            {
                // Track an explicit `:authorinitials:` entry so a `:author:`
                // entry (whether it precedes or follows this one) does not
                // overwrite it with initials re-derived from the author's name.
                // Asciidoctor preserves an explicit `authorinitials` for a
                // single `author`; an empty value (`:authorinitials:`) still
                // counts as explicit, only an unset (`:authorinitials!:`) does
                // not.
                if attr
                    .item
                    .name()
                    .data()
                    .eq_ignore_ascii_case("authorinitials")
                {
                    authorinitials_from_entry =
                        !matches!(attr.item.value(), InterpretedValue::Unset);
                }

                // Special handling for :author: attribute to populate individual author
                // attributes.
                //
                // When the value is a plain name, the partitioned name replaces
                // the stored `author` value. This condenses repeated interior
                // whitespace and joins a name with four or more parts, matching
                // Asciidoctor (issue #758).
                let mut author_name_override: Option<String> = None;
                if attr.item.name().data().eq_ignore_ascii_case("author")
                    && let Some(raw_value) = attr.item.raw_value()
                    && let Some(author) = Author::parse(raw_value.data(), parser, true)
                {
                    // Set individual author attributes.
                    parser.set_attribute_by_value_from_header("firstname", author.firstname());
                    if let Some(middlename) = author.middlename() {
                        parser.set_attribute_by_value_from_header("middlename", middlename);
                    }
                    if let Some(lastname) = author.lastname() {
                        parser.set_attribute_by_value_from_header("lastname", lastname);
                    }

                    // Do not re-derive `authorinitials` when the document has
                    // supplied its own via an explicit entry (see above).
                    if !authorinitials_from_entry {
                        parser.set_attribute_by_value_from_header(
                            "authorinitials",
                            author.initials(),
                        );
                    }

                    if let Some(email) = author.email() {
                        parser.set_attribute_by_value_from_header("email", email);
                    }

                    // Only override the `author` value when the name was
                    // partitioned by the fallback whitespace split — a plain name
                    // that does not match the author pattern (four or more parts,
                    // or punctuation such as a comma). A value that matches the
                    // pattern, carries an inline email (`<…>`), or holds an
                    // attribute reference (`{…}`) keeps the substituted entry
                    // value set below, so the handling of those forms is
                    // unchanged.
                    let raw = raw_value.data();
                    if !raw.contains('<') && !raw.contains('{') && !matches_author_pattern(raw) {
                        author_name_override = Some(author.name().to_string());
                    }

                    // Retain the author parsed from the raw (pre-substitution)
                    // value so the resolved author list does not have to
                    // re-parse the HTML-encoded `author` attribute. A later
                    // `:author:` entry overrides an earlier one.
                    author_attribute = Some(author);
                }

                parser.set_attribute_from_header(&attr.item, &mut warnings);

                if let Some(author_name) = author_name_override {
                    parser.set_attribute_by_value_from_header("author", author_name);
                }

                // A `:doctitle:` entry below the document title is a candidate to
                // override the implicit section title (resolved after the header
                // is fully parsed; see below).
                if title.is_some() && attr.item.name().data().eq_ignore_ascii_case("doctitle") {
                    doctitle_entry_after_title = true;
                }

                attributes.push(attr.item);
                source = attr.after;
            } else if title.is_none()
                && line.starts_with('[')
                && line.ends_with(']')
                && document_title_follows_block_metadata(line_mi.after)
                && let Some((metadata, metadata_warnings)) =
                    parse_document_metadata_attrlist(line, parser)
            {
                warnings.extend(metadata_warnings);
                // A block attribute line directly above the document title assigns
                // metadata to the *document* — its `id`, `reftext`, `role`, and
                // options — mirroring Asciidoctor's `parse_document_header`. Each
                // recognized value folds into the document's attributes at this
                // point in the header, so it follows document order alongside any
                // equivalent header attribute entry (e.g. `:reftext:`).
                //
                // A `separator` sets the subtitle separator; it behaves exactly
                // like assigning the `title-separator` document attribute here, so
                // both mechanisms share the same partitioning logic.
                //
                // The line is only intercepted when a document title eventually
                // follows — possibly after further stacked block attribute lines,
                // each folded on its own pass through this loop, mirroring
                // Asciidoctor's `parse_block_metadata_lines`. Otherwise it is
                // block metadata for the body (e.g. a table's `separator`) and is
                // left for the block parser.
                if let Some(doc_id) = metadata.id {
                    id = Some(doc_id);
                }
                if let Some(separator) = metadata.separator {
                    parser.set_attribute_by_value_from_header("title-separator", separator);
                }
                if let Some(reftext) = metadata.reftext {
                    parser.set_attribute_by_value_from_header("reftext", reftext);
                }
                if !metadata.roles.is_empty() {
                    // Fold the role(s) into the `role` document attribute
                    // (space-joined, as Asciidoctor stores `attributes['role']`)
                    // and also retain them on the header so `Document::roles()`
                    // agrees with the document attribute (see #820). Roles from
                    // separate stacked block attribute lines accumulate, just as
                    // multiple roles within a single line combine (see #821).
                    roles.extend(metadata.roles);
                    parser.set_attribute_by_value_from_header("role", roles.join(" "));
                }
                for option in metadata.options {
                    parser.set_attribute_by_value_from_header(format!("{option}-option"), "");
                }
                source = line_mi.after;
            } else if title.is_none()
                && let Some(marker) = document_title_marker(line)
            {
                // Strip an optional symmetric close (a trailing ` =` or ` #`
                // matching the single opening marker), mirroring section titles.
                let title_span = crate::blocks::strip_symmetric_title_close(
                    line.discard(2).discard_whitespace(),
                    marker,
                    1,
                );
                saw_implicit_title = true;

                title_source = Some(title_span);

                // A `doctitle` attribute already set above the title – via a
                // `:doctitle:` entry or the API – overrides the implicit title:
                // the implicit text is discarded, the existing doctitle stands
                // as the document title, and the `doctitle` attribute is left
                // untouched. Otherwise the implicit title is
                // substituted now (so `{doctitle}` references below resolve to
                // it) and recorded as the baseline for a later override check.
                if let InterpretedValue::Value(existing) = parser.attribute_value("doctitle")
                    && !existing.is_empty()
                {
                    implicit_overridden_from_above = true;
                    implicit_doctitle_str = Some(existing.clone());
                    title = Some(existing);
                } else {
                    let title_str = apply_header_subs(title_span.data(), parser);

                    parser.set_attribute_by_value_from_header("doctitle", &title_str);

                    implicit_doctitle_str = Some(title_str.clone());
                    title = Some(title_str);
                }

                source = line_mi.after;
            } else if title.is_some() && author_line.is_none() {
                author_line = Some(AuthorLine::parse(line, parser));
                source = line_mi.after;
            } else if title.is_some() && author_line.is_some() && revision_line.is_none() {
                revision_line = Some(RevisionLine::parse(line, parser));
                source = line_mi.after;
            } else {
                if title.is_some() {
                    warnings.push(Warning {
                        source: line,
                        warning: WarningType::DocumentHeaderNotTerminated,
                        origin: None,
                    });
                }
                break;
            }
        }

        let after = source.discard_empty_lines();
        let source = original_source.trim_remainder(source);

        // Finalize the document (section) title, mirroring Asciidoctor's
        // `parse_document_header` doctitle handling. The `doctitle` attribute
        // retains the eager, at-title-line substitution; the section title below
        // is (re)derived from the *final* attribute state so that:
        //
        //   - an implicit `= Title` referencing an attribute defined later in the
        //     header still resolves ("lazy" resolution),
        //   - a `:doctitle:` attribute entry (above or below the title, or in a
        //     document with no title line at all) can supply or override it.
        let final_doctitle_attr = match parser.attribute_value("doctitle") {
            InterpretedValue::Value(v) if !v.is_empty() => Some(v),
            _ => None,
        };

        title = if saw_implicit_title {
            // The base section title is normally the eager (at-title-line)
            // substitution already held in `title`. It is re-resolved against the
            // final attribute set only when that eager substitution left an
            // unresolved attribute reference – an attribute defined later in the
            // header – so that one-shot substitutions such as a `{counter:…}` in
            // the title are not evaluated a second time. When the implicit title
            // was overridden by a `doctitle` set above it, `title` already holds
            // that (resolved) value and is not re-substituted.
            //
            // Residual edge: a title that *mixes* a counter with a later-defined
            // reference (e.g. `= {counter:n} {project-name}`) still contains a
            // `{` after the eager pass, so the re-resolution runs and advances the
            // counter a second time. Re-resolving is done from the raw title (not
            // the eager result) so that escaped `\{…}` and specialchars stay
            // correct; the counter here is the price of that. This is a rare
            // combination and no test exercises it.
            let base = if !implicit_overridden_from_above
                && let Some(raw) = title_source
                && implicit_doctitle_str
                    .as_deref()
                    .is_some_and(|s| s.contains('{'))
            {
                Some(apply_header_subs(raw.data(), parser))
            } else {
                title
            };

            // A `:doctitle:` entry below the title overrides the section title
            // when it sets a new, non-empty value (an empty or unchanged value
            // leaves the implicit title in place).
            if doctitle_entry_after_title
                && let Some(ref dt) = final_doctitle_attr
                && Some(dt) != implicit_doctitle_str.as_ref()
            {
                Some(dt.clone())
            } else {
                base
            }
        } else {
            // No `= Title` line: a `:doctitle:` attribute entry, if any, supplies
            // the implicit document title.
            final_doctitle_attr
        };

        // Partition the (fully substituted) document title into a main title and
        // an optional subtitle. This happens after the header has been fully
        // parsed so that a `title-separator` attribute takes effect even when it
        // is defined below the document title line.
        let (main_title, subtitle) = match &title {
            Some(title) => {
                let (main_title, subtitle) = partition_title(title, parser);
                (Some(main_title), subtitle)
            }
            None => (None, None),
        };

        // The value returned by `Document::doctitle()`: a `title` attribute entry
        // overrides the section title (Asciidoctor's `Document#doctitle`), even
        // when it is blank; otherwise the section title is the doctitle.
        let doctitle = match parser.attribute_value("title") {
            InterpretedValue::Value(v) => Some(v),
            InterpretedValue::Set => Some(String::new()),
            InterpretedValue::Unset => title.clone(),
        };

        // Resolve the document's author list. The author line, when present, is
        // the source of truth; otherwise the list is derived from the `author`,
        // `authors`, and indexed `author_N` document attributes (see
        // [`resolve_authors`]). Those attributes can only be set by header
        // attribute entries, so a header with none needs no reconciliation.
        let authors = resolve_authors(
            author_line.as_ref(),
            author_attribute,
            !attributes.is_empty(),
            parser,
        );

        // Asciidoctor exposes the number of resolved authors via the
        // `authorcount` document attribute. It defaults to `0` (a built-in
        // default), so only a non-zero count is materialized here — this keeps an
        // author-less parse from touching the attribute map at all.
        if !authors.is_empty() {
            parser.set_attribute_by_value_from_header("authorcount", authors.len().to_string());
        }

        MatchAndWarnings {
            item: MatchedItem {
                item: Self {
                    title_source,
                    title,
                    doctitle,
                    main_title,
                    subtitle,
                    id,
                    roles,
                    attributes,
                    author_line,
                    authors,
                    revision_line,
                    comments,
                    source: source.trim_trailing_whitespace(),
                },
                after,
            },
            warnings,
        }
    }

    /// Return a [`Span`] describing the raw document title, if there was one.
    pub fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    /// Return the document's title, if there was one, having applied header
    /// substitutions.
    ///
    /// If the title contains a subtitle (see [`subtitle`]), this returns the
    /// full, combined title. Use [`main_title`] to obtain only the portion
    /// preceding the subtitle.
    ///
    /// [`subtitle`]: Self::subtitle
    /// [`main_title`]: Self::main_title
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Return the effective document title, applying the override precedence of
    /// Asciidoctor's `Document#doctitle`: a `title` attribute entry (even a
    /// blank one) takes priority over the section [`title`], which in turn may
    /// have been supplied or overridden by a `:doctitle:` attribute entry.
    ///
    /// This backs [`Document::doctitle`] and can differ from [`title`]: for
    /// `= Document Title` followed by `:title: Override`, [`title`] is
    /// `Document Title` while this is `Override`.
    ///
    /// [`title`]: Self::title
    /// [`Document::doctitle`]: crate::Document::doctitle
    pub(crate) fn doctitle(&self) -> Option<&str> {
        self.doctitle.as_deref()
    }

    /// Return the main portion of the document title, if there was a title.
    ///
    /// When the document title contains a subtitle separator (a colon followed
    /// by a space, by default), the title is partitioned into a main title and
    /// a [`subtitle`]. This returns the portion preceding the final separator.
    /// When there is no subtitle, this is identical to [`title`].
    ///
    /// [`subtitle`]: Self::subtitle
    /// [`title`]: Self::title
    pub fn main_title(&self) -> Option<&str> {
        self.main_title.as_deref()
    }

    /// Return the document's subtitle, if the title contained one.
    ///
    /// A subtitle is the text following the final subtitle separator in the
    /// document title. The separator defaults to a colon followed by a space
    /// (`:{sp}`) and can be overridden with the `title-separator` document
    /// attribute. Returns `None` when the title has no subtitle.
    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    /// Return the document's ID, if one was assigned.
    ///
    /// A document ID is set with a block attribute line directly above the
    /// document title, using either the shorthand (`[#id]`) or longhand
    /// (`[id=id]`) syntax. Returns `None` when no such ID was given.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Return the document's role(s), if any were assigned.
    ///
    /// Roles are set with a block attribute line directly above the document
    /// title, using either the shorthand (`[.role]`) or longhand (`[role=…]`)
    /// syntax; multiple roles combine. The same role(s) are also folded into
    /// the `role` document attribute (space-joined), so the block accessor and
    /// the document attribute agree. Returns an empty vector when no role was
    /// given.
    pub fn roles(&self) -> Vec<&str> {
        self.roles.iter().map(String::as_str).collect()
    }

    /// Return an iterator over the attributes in this header.
    pub fn attributes(&'src self) -> Iter<'src, Attribute<'src>> {
        self.attributes.iter()
    }

    /// Returns the author line, if found.
    pub fn author_line(&self) -> Option<&AuthorLine<'src>> {
        self.author_line.as_ref()
    }

    /// Returns the document's authors.
    ///
    /// Authors may be declared on the [author line] or via the `author` /
    /// `author_N` (and companion `email_N`, …) document attributes; this
    /// returns the resolved list regardless of which mechanism was used. When
    /// the document has no author information, the slice is empty.
    ///
    /// [author line]: https://docs.asciidoctor.org/asciidoc/latest/document/author-line/
    pub fn authors(&self) -> &[Author] {
        &self.authors
    }

    /// Returns the revision line, if found.
    pub fn revision_line(&self) -> Option<&RevisionLine<'src>> {
        self.revision_line.as_ref()
    }

    /// Return an iterator over the comments in this header.
    pub fn comments(&'src self) -> Iter<'src, Span<'src>> {
        self.comments.iter()
    }
}

impl<'src> HasSpan<'src> for Header<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

/// If `line` opens a `////` block comment, consume the comment block and
/// return the source position immediately after its closing delimiter together
/// with a flag reporting whether a closing delimiter was found; otherwise
/// return `None`.
///
/// A block comment delimiter is a line of four or more forward slashes and
/// nothing else, matching Asciidoctor's comment-block delimiter (a line of
/// exactly three slashes instead terminates the header and is handled by the
/// caller). The closing delimiter must repeat the opening line exactly; when
/// it is absent the block runs to the end of the input, mirroring
/// Asciidoctor's `read_lines_until`, and the returned flag is `false` so the
/// caller can warn that the comment block was never terminated.
///
/// `after` is the source immediately following `line` (the opening delimiter).
fn skip_block_comment<'src>(line: Span<'src>, after: Span<'src>) -> Option<(Span<'src>, bool)> {
    let delimiter = line.data();
    if delimiter.len() < 4 || !delimiter.bytes().all(|b| b == b'/') {
        return None;
    }

    let mut next = after;
    let mut terminated = false;
    while !next.is_empty() {
        let line_mi = next.take_normalized_line();
        next = line_mi.after;
        if line_mi.item.data() == delimiter {
            terminated = true;
            break;
        }
    }

    Some((next, terminated))
}

/// Returns the ATX marker character that introduces `line` as a document
/// title, or `None` if the line is not a document title.
///
/// Both the AsciiDoc marker (`=`) and the Markdown-style marker (`#`) are
/// accepted, mirroring the alternation at the head of Asciidoctor's
/// section-title regex. The marker character is returned so the caller can
/// require a symmetric close to use the same marker.
fn document_title_marker(line: Span<'_>) -> Option<char> {
    if line.starts_with("= ") {
        Some('=')
    } else if line.starts_with("# ") {
        Some('#')
    } else {
        None
    }
}

/// Reports whether a document title marker eventually follows the source at
/// `after`, allowing any number of stacked block attribute lines in between.
///
/// Starting immediately below the block attribute line under consideration,
/// consecutive lines that are themselves document-metadata block attribute
/// lines (see [`is_document_metadata_line`]) are skipped, and the first line
/// that is not is tested for a document title marker. This generalizes the
/// original single-line lookahead so that stacked metadata lines above the
/// title are all folded, mirroring Asciidoctor's `parse_block_metadata_lines`.
///
/// A line that starts with `[` and ends with `]` but is *not* a valid
/// document-metadata line (e.g. a `[[anchor]]` block anchor or a leading-space
/// form) stops the scan without matching, so the run of foldable lines is only
/// ever a contiguous prefix of well-formed metadata lines terminated by the
/// title.
fn document_title_follows_block_metadata(after: Span<'_>) -> bool {
    let mut next = after;

    while !next.is_empty() {
        let line_mi = next.take_normalized_line();
        let line = line_mi.item;

        if document_title_marker(line).is_some() {
            return true;
        }

        if !is_document_metadata_line(line) {
            return false;
        }

        next = line_mi.after;
    }

    false
}

/// Reports whether `line` is a block attribute line that this crate folds into
/// document metadata when it appears above the document title.
///
/// This captures the purely syntactic acceptance rules shared by the lookahead
/// ([`document_title_follows_block_metadata`]) and the folding step
/// ([`parse_document_metadata_attrlist`]): the line must be bracket-delimited
/// and its contents must not be empty, must not begin with whitespace, and must
/// not be a `[[anchor]]` block anchor. The legacy double-bracket anchor above
/// the document title is left unsupported (it terminates the header, as
/// before); the single-bracket `[#id]` shorthand is the supported way to set a
/// document ID.
fn is_document_metadata_line(line: Span<'_>) -> bool {
    if !(line.starts_with('[') && line.ends_with(']')) {
        return false;
    }

    let inner = line.slice(1..line.len() - 1);

    !(inner.is_empty()
        || inner.starts_with(' ')
        || inner.starts_with('\t')
        || (inner.starts_with('[') && inner.ends_with(']')))
}

/// Document metadata folded from a block attribute line appearing directly
/// above the document title.
///
/// Each field holds an already-owned copy of a recognized value, so the caller
/// can apply them without borrowing the (dropped) attribute list.
struct DocumentMetadata {
    id: Option<String>,
    separator: Option<String>,
    reftext: Option<String>,
    roles: Vec<String>,
    options: Vec<String>,
}

/// Parse a block attribute line (e.g. `[reftext="…"]`, `[#id]`, `[role=…]`,
/// `[separator=::]`) appearing above the document title into
/// [`DocumentMetadata`].
///
/// The `line` is expected to begin with `[` and end with `]`. Returns the
/// folded metadata together with any warnings raised while parsing the
/// attribute list when the line is a well-formed block attribute list, and
/// `None` otherwise (so the caller can fall through to its normal handling of
/// the line, which then terminates the header). The warnings are only surfaced
/// when the line is actually consumed as document metadata; otherwise the line
/// is left for the block parser, which reports them on its own path.
fn parse_document_metadata_attrlist<'src>(
    line: Span<'src>,
    parser: &Parser,
) -> Option<(DocumentMetadata, Vec<Warning<'src>>)> {
    // Reject forms that are not block attribute lists (a leading space or tab,
    // an empty list, or a `[[anchor]]` block anchor); see
    // [`is_document_metadata_line`] for the shared acceptance rules. The caller
    // has already confirmed the enclosing square brackets are present.
    if !is_document_metadata_line(line) {
        return None;
    }

    // Drop the enclosing square brackets.
    let inner = line.slice(1..line.len() - 1);

    let MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: _,
        },
        warnings,
    } = Attrlist::parse(inner, parser, AttrlistContext::Block);

    let metadata = DocumentMetadata {
        id: attrlist.id().map(str::to_string),
        separator: attrlist
            .named_attribute("separator")
            .map(|attr| attr.value().to_string()),
        reftext: attrlist
            .named_attribute("reftext")
            .map(|attr| attr.value().to_string()),
        roles: attrlist.roles().iter().map(|r| r.to_string()).collect(),
        options: attrlist.options().iter().map(|o| o.to_string()).collect(),
    };

    Some((metadata, warnings))
}

/// Partition a document title into its main title and optional subtitle.
///
/// The separator is the value of the `title-separator` document attribute
/// (defaulting to `:`) with a single space appended. The separator is searched
/// for from the end of the title, so only the last occurrence partitions the
/// title. When the separator is not present, the entire title is the main
/// title and there is no subtitle.
fn partition_title(title: &str, parser: &Parser) -> (String, Option<String>) {
    // Read the configured `title-separator` document attribute directly. Unlike
    // `Parser::attribute_value`, this bypasses the counter overlay: the title
    // separator is a configuration attribute, never a counter, and Asciidoctor
    // likewise resolves it with a plain attribute lookup.
    let separator = match parser.effective_attribute("title-separator") {
        Some(av) => match &av.value {
            InterpretedValue::Value(value) if !value.is_empty() => value.clone(),
            _ => ":".to_string(),
        },
        None => ":".to_string(),
    };

    let separator = format!("{separator} ");

    match title.rfind(&separator) {
        Some(index) => {
            let main_title = title[..index].to_string();
            let subtitle = title[index + separator.len()..].to_string();
            (main_title, Some(subtitle))
        }
        None => (title.to_string(), None),
    }
}

/// Resolves the document's author list.
///
/// When an [`AuthorLine`] is present it is authoritative (and has already
/// populated the `author_N` attributes). Otherwise the list is reconstructed
/// from document attributes, mirroring Asciidoctor's `parse_header_metadata`
/// reconciliation in precedence order: a directly-assigned `author` attribute
/// stands in for a single author; failing that a semicolon-separated `authors`
/// attribute is split into individual authors; and failing that a contiguous
/// run of indexed `author_N` attributes (`author_1`, `author_2`, …) each
/// contributes one author. In each case the email is taken from the companion
/// `email`/`email_N` attribute, reflecting its final value.
///
/// For the `authors` and `author_N` forms this also populates the derived
/// author attributes (`author`, `firstname`, `authorinitials`, the `authors`
/// list, the per-author `author_N` companions, …) so that references such as
/// `{author}` resolve, matching Asciidoctor's `process_authors` (see issue
/// #718).
///
/// `author_attribute` is the author already parsed from the raw `author`
/// attribute value (see the header parse loop); it is reused rather than
/// re-parsing the HTML-encoded stored value.
///
/// `header_has_attributes` reports whether the header carried any attribute
/// entries. When it did not, none of the `author` / `authors` / `author_N`
/// attributes can be set, so the attribute lookups are skipped — the common
/// case for a document whose header is just a title (or absent).
fn resolve_authors(
    author_line: Option<&AuthorLine>,
    author_attribute: Option<Author>,
    header_has_attributes: bool,
    parser: &mut Parser,
) -> Vec<Author> {
    if let Some(author_line) = author_line {
        return author_line.authors().cloned().collect();
    }

    if !header_has_attributes {
        return vec![];
    }

    // A directly-assigned `author` attribute describes a single author — but
    // only while it remains set. A later `:author!:` unsets the attribute
    // without carrying a raw value to refresh `author_attribute`, so consult
    // the attribute's final state rather than trusting the cached parse. The
    // per-author attributes for this form were already populated inline as the
    // `:author:` entry was parsed.
    if attribute_string(parser, "author").is_some()
        && let Some(author) = author_attribute
    {
        return vec![author.with_email(attribute_string(parser, "email"))];
    }

    // A semicolon-separated `authors` attribute entry contributes one author
    // per entry (Asciidoctor's `process_authors` with `multiple` set).
    if let Some(authors_value) = attribute_string(parser, "authors") {
        let authors = collect_indexed_authors(
            split_author_entries(&authors_value)
                .into_iter()
                .filter_map(|entry| Author::parse(entry, parser, true)),
            parser,
        );

        if !authors.is_empty() {
            set_author_metadata(parser, &authors);
            return authors;
        }
    }

    // Otherwise, walk the indexed `author_N` attributes until one is missing.
    let mut raw_names = vec![];
    let mut index = 1;

    while let Some(name) = attribute_string(parser, &format!("author_{index}")) {
        raw_names.push(name);
        index += 1;
    }

    let authors = collect_indexed_authors(
        raw_names
            .iter()
            .filter_map(|name| Author::parse(name, parser, true)),
        parser,
    );

    if !authors.is_empty() {
        set_author_metadata(parser, &authors);
    }

    authors
}

/// Reads the string value of a document attribute, or `None` when it is unset
/// or set without a value.
fn attribute_string(parser: &Parser, name: &str) -> Option<String> {
    match parser.attribute_value(name) {
        InterpretedValue::Value(value) => Some(value),
        _ => None,
    }
}

/// Attaches each parsed author's companion `email_N` attribute (`email_1` for
/// the first author, `email_2` for the second, …) so the resolved list carries
/// the emails supplied through separate attribute entries.
fn collect_indexed_authors(authors: impl Iterator<Item = Author>, parser: &Parser) -> Vec<Author> {
    authors
        .enumerate()
        .map(|(idx, author)| {
            author.with_email(attribute_string(parser, &format!("email_{}", idx + 1)))
        })
        .collect()
}

/// Splits an `authors` attribute value into raw author entries.
///
/// A semicolon separates authors only when it is immediately followed by a
/// space or the end of the value, matching Asciidoctor's `AuthorDelimiterRx`
/// (`/;(?: |$)/`). Blank entries are left in place; [`Author::parse`] trims
/// each entry and discards the empty ones.
fn split_author_entries(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut entries: Vec<&str> = Vec::new();
    let mut start = 0;

    for (index, c) in value.char_indices() {
        if c != ';' {
            continue;
        }

        let is_separator = match bytes.get(index + 1) {
            Some(next) => *next == b' ',
            None => true,
        };

        if is_separator {
            entries.push(&value[start..index]);
            start = index + 1;
        }
    }

    entries.push(&value[start..]);
    entries
}

/// Populates the derived author document attributes from a resolved author
/// list, mirroring Asciidoctor's `process_authors`.
///
/// The first author sets the unsuffixed keys (`author`, `firstname`,
/// `authorinitials`, …); each subsequent author sets its `_N` companions. Once
/// a second author appears, the first author is also mirrored onto its `_1`
/// companions. The `authors` attribute is rewritten to the comma-joined list of
/// resolved author names.
///
/// This intentionally does **not** honor `authorinitials_from_entry`: the
/// derived `authorinitials` always overwrites an explicit `:authorinitials:`
/// entry for the `authors` and `author_N` forms. Only a single `:author:` entry
/// (handled inline in [`Header::parse`]) preserves an explicit override,
/// exactly as Asciidoctor does — its `authorinitials` deletion guard lives only
/// in the `author` branch of `process_authors`, not the `authors`/indexed
/// branches.
fn set_author_metadata(parser: &mut Parser, authors: &[Author]) {
    for (idx, author) in authors.iter().enumerate() {
        set_author_keys(parser, author, if idx == 0 { None } else { Some(idx + 1) });

        // The `_1` companions are only assigned once a second author is seen.
        if idx == 1
            && let Some(first) = authors.first()
        {
            set_author_keys(parser, first, Some(1));
        }
    }

    let joined = authors
        .iter()
        .map(Author::name)
        .collect::<Vec<_>>()
        .join(", ");

    parser.set_attribute_by_value_from_header("authors", joined);
}

/// Sets the author attributes for a single author, either as the unsuffixed
/// keys (`index` is `None`) or the `_N` companions (`index` is `Some(n)`).
fn set_author_keys(parser: &mut Parser, author: &Author, index: Option<usize>) {
    let key = |name: &str| match index {
        None => name.to_string(),
        Some(n) => format!("{name}_{n}"),
    };

    parser.set_attribute_by_value_from_header(key("author"), author.name());
    parser.set_attribute_by_value_from_header(key("firstname"), author.firstname());

    if let Some(middlename) = author.middlename() {
        parser.set_attribute_by_value_from_header(key("middlename"), middlename);
    }

    if let Some(lastname) = author.lastname() {
        parser.set_attribute_by_value_from_header(key("lastname"), lastname);
    }

    parser.set_attribute_by_value_from_header(key("authorinitials"), author.initials());

    if let Some(email) = author.email() {
        parser.set_attribute_by_value_from_header(key("email"), email);
    }
}

fn apply_header_subs(source: &str, parser: &Parser) -> String {
    let span = Span::new(source);

    let mut content = Content::from(span);
    SubstitutionGroup::Header.apply(&mut content, parser, None);

    content.rendered().to_string()
}

impl std::fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("doctitle", &self.doctitle)
            .field("main_title", &self.main_title)
            .field("subtitle", &self.subtitle)
            .field("id", &self.id)
            .field("roles", &self.roles)
            .field("attributes", &DebugSliceReference(&self.attributes))
            .field("author_line", &self.author_line)
            .field("authors", &self.authors)
            .field("revision_line", &self.revision_line)
            .field("comments", &DebugSliceReference(&self.comments))
            .field("source", &self.source)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::prelude::*;

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();

        let h1 = crate::document::Header::parse(crate::Span::new("= Title"), &mut parser)
            .unwrap_if_no_warnings();
        let h2 = h1.clone();

        assert_eq!(h1, h2);
    }

    #[test]
    fn only_title() {
        let mut parser = Parser::default();
        let mi = crate::document::Header::parse(crate::Span::new("= Just the Title"), &mut parser)
            .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: Some(Span {
                    data: "Just the Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                }),
                title: Some("Just the Title"),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Just the Title",
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
                col: 17,
                offset: 16
            }
        );
    }

    #[test]
    fn trims_leading_spaces_in_title() {
        // This is totally a judgement call on my part. As far as I can tell,
        // the language doesn't describe behavior here.
        let mut parser = Parser::default();
        let mi =
            crate::document::Header::parse(crate::Span::new("=    Just the Title"), &mut parser)
                .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: Some(Span {
                    data: "Just the Title",
                    line: 1,
                    col: 6,
                    offset: 5,
                }),
                title: Some("Just the Title"),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "=    Just the Title",
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
                col: 20,
                offset: 19
            }
        );
    }

    #[test]
    fn trims_trailing_spaces_in_title() {
        let mut parser = Parser::default();
        let mi =
            crate::document::Header::parse(crate::Span::new("= Just the Title   "), &mut parser)
                .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: Some(Span {
                    data: "Just the Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                }),
                title: Some("Just the Title"),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Just the Title",
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
                col: 20,
                offset: 19
            }
        );
    }

    #[test]
    fn title_and_attribute() {
        let mut parser = Parser::default();

        let mi = crate::document::Header::parse(
            crate::Span::new("= Just the Title\n:foo: bar\n\nblah"),
            &mut parser,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: Some(Span {
                    data: "Just the Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                }),
                title: Some("Just the Title"),
                attributes: &[Attribute {
                    name: Span {
                        data: "foo",
                        line: 2,
                        col: 2,
                        offset: 18,
                    },
                    value_source: Some(Span {
                        data: "bar",
                        line: 2,
                        col: 7,
                        offset: 23,
                    }),
                    value: InterpretedValue::Value("bar"),
                    source: Span {
                        data: ":foo: bar",
                        line: 2,
                        col: 1,
                        offset: 17,
                    }
                }],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Just the Title\n:foo: bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 4,
                col: 1,
                offset: 28
            }
        );
    }

    #[test]
    fn title_applies_header_substitutions() {
        let mut parser = Parser::default();

        let mi = crate::document::Header::parse(
            crate::Span::new("= The Title & Some{sp}Nonsense\n:foo: bar\n\nblah"),
            &mut parser,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: Some(Span {
                    data: "The Title & Some{sp}Nonsense",
                    line: 1,
                    col: 3,
                    offset: 2,
                }),
                title: Some("The Title &amp; Some Nonsense"),
                attributes: &[Attribute {
                    name: Span {
                        data: "foo",
                        line: 2,
                        col: 2,
                        offset: 32,
                    },
                    value_source: Some(Span {
                        data: "bar",
                        line: 2,
                        col: 7,
                        offset: 37,
                    }),
                    value: InterpretedValue::Value("bar"),
                    source: Span {
                        data: ":foo: bar",
                        line: 2,
                        col: 1,
                        offset: 31,
                    }
                }],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Title & Some{sp}Nonsense\n:foo: bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 4,
                col: 1,
                offset: 42
            }
        );
    }

    #[test]
    fn attribute_without_title() {
        let mut parser = Parser::default();
        let mi = crate::document::Header::parse(crate::Span::new(":foo: bar\n\nblah"), &mut parser)
            .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Header {
                title_source: None,
                title: None,
                attributes: &[Attribute {
                    name: Span {
                        data: "foo",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    value_source: Some(Span {
                        data: "bar",
                        line: 1,
                        col: 7,
                        offset: 6,
                    }),
                    value: InterpretedValue::Value("bar"),
                    source: Span {
                        data: ":foo: bar",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                }],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: ":foo: bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 3,
                col: 1,
                offset: 11
            }
        );
    }

    #[test]
    fn sets_doctitle_attribute() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title Goes Here");

        assert_eq!(
            parser.attribute_value("doctitle"),
            InterpretedValue::Value("Document Title Goes Here")
        );
    }

    #[test]
    fn sets_author_attributes_from_author_attribute() {
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: John Q. Smith <john@example.com>");

        // Verify that individual author attributes are set.
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("John")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Q.")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Smith")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("JQS")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("john@example.com")
        );

        // Also verify the original author attribute is still set (with HTML encoding).
        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("John Q. Smith &lt;john@example.com&gt;")
        );
    }

    #[test]
    fn author_attribute_with_four_or_more_parts_is_partitioned() {
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/758: a value
        // with more than three parts does not match the author pattern, so it is
        // partitioned by splitting on whitespace into at most three parts. The
        // trailing parts are assigned to `lastname` and repeated interior
        // whitespace is condensed.
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Leroy  Harold  Scherer,  Jr.");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Leroy Harold Scherer, Jr.")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Leroy")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Harold")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Scherer, Jr.")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("LHS")
        );
    }

    #[test]
    fn author_attribute_two_part_fallback_partitions_lastname() {
        // A two-part value that does not match the author pattern (here because
        // of the comma attached to the first part) still partitions into a first
        // and last name via the whitespace split.
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Jane, Doe");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Jane, Doe")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Jane,")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Doe")
        );
    }

    #[test]
    fn author_attribute_single_part_fallback_is_firstname_only() {
        // A single-token value that does not match the author pattern partitions
        // to `firstname` alone, with no middle or last name.
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Jane,");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Jane,")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Jane,")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(parser.attribute_value("lastname"), InterpretedValue::Unset);
    }

    #[test]
    fn author_attribute_four_or_more_parts_with_inline_email() {
        // A four-plus-part fallback value that carries a trailing `<email>` must
        // split the email off before partitioning, so it lands in `email` rather
        // than being absorbed into `lastname`.
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Leroy  Harold  Scherer,  Jr. <leroy@example.com>");

        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Leroy")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Harold")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Scherer, Jr.")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("leroy@example.com")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("LHS")
        );
    }

    #[test]
    fn author_attribute_reference_expands_and_partitions() {
        // A `:author:` value given entirely as an attribute reference is expanded
        // and then partitioned by the names-only rules, so it yields the same
        // metadata as the equivalent literal four-plus-part name.
        let mut parser = Parser::default();
        let _doc = parser.parse(":full-name: Leroy Harold Scherer, Jr.\n:author: {full-name}");

        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Leroy")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Harold")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Scherer, Jr.")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("LHS")
        );
    }

    #[test]
    fn author_attribute_reference_within_larger_value_expands_and_partitions() {
        // The same partitioning applies when the reference is only part of the
        // value (so the single-attribute fast path is not taken) and the expanded
        // result still fails the author pattern.
        let mut parser = Parser::default();
        let _doc = parser.parse(":rest: Harold Scherer, Jr.\n:author: Leroy {rest}");

        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Leroy")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Harold")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Scherer, Jr.")
        );
    }

    #[test]
    fn author_attribute_non_breaking_space_is_not_a_name_separator() {
        // Only ASCII whitespace separates name parts. A non-breaking space
        // (U+00A0) joining two words keeps them as a single first name, matching
        // Ruby's whitespace split.
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: John\u{a0}Doe Scherer, Jr.");

        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("John\u{a0}Doe")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Scherer,")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Jr.")
        );
    }

    #[test]
    fn sets_author_attributes_from_author_attribute_two_names() {
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Jane Doe");

        // Verify that individual author attributes are set.
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Jane")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Doe")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("JD")
        );
        assert_eq!(parser.attribute_value("email"), InterpretedValue::Unset);
    }

    #[test]
    fn sets_author_attributes_from_author_attribute_single_name() {
        let mut parser = Parser::default();
        let _doc = parser.parse(":author: Cher");

        // Verify that individual author attributes are set.
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Cher")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(parser.attribute_value("lastname"), InterpretedValue::Unset);
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("C")
        );
        assert_eq!(parser.attribute_value("email"), InterpretedValue::Unset);
    }

    #[test]
    fn sets_author_attributes_from_empty_string() {
        let mut parser = Parser::default();
        let _doc = parser.parse(":author:");

        // Verify that individual author attributes are set.
        assert_eq!(parser.attribute_value("firstname"), InterpretedValue::Unset);
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(parser.attribute_value("lastname"), InterpretedValue::Unset);
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Unset
        );
        assert_eq!(parser.attribute_value("email"), InterpretedValue::Unset);

        assert_eq!(parser.attribute_value("author"), InterpretedValue::Set);
    }

    #[test]
    fn authors_from_author_line() {
        let doc = Parser::default().parse("= Title\nKismet R. Lee <kismet@asciidoctor.org>");

        assert_eq!(doc.authors().len(), 1);

        let author = doc.authors().first().unwrap();
        assert_eq!(author.name(), "Kismet R. Lee");
        assert_eq!(author.email(), Some("kismet@asciidoctor.org"));
        assert_eq!(author.initials(), "KRL");
    }

    #[test]
    fn authors_from_author_attribute() {
        // With no author line, a directly-assigned `author` attribute stands in
        // for a single author, taking its email from the `email` attribute.
        let doc =
            Parser::default().parse("= Title\n:author: Jane Q. Public\n:email: jane@example.com");

        assert_eq!(doc.authors().len(), 1);

        let author = doc.authors().first().unwrap();
        assert_eq!(author.name(), "Jane Q. Public");
        assert_eq!(author.firstname(), "Jane");
        assert_eq!(author.middlename(), Some("Q."));
        assert_eq!(author.lastname(), Some("Public"));
        assert_eq!(author.email(), Some("jane@example.com"));
        assert_eq!(author.initials(), "JQP");
    }

    #[test]
    fn authors_from_author_attribute_with_inline_email() {
        // The email may be given inline in the `author` attribute value; the
        // resolved author reflects the parsed name and that email.
        let doc = Parser::default().parse("= Title\n:author: John Q. Smith <john@example.com>");

        assert_eq!(doc.authors().len(), 1);

        let author = doc.authors().first().unwrap();
        assert_eq!(author.name(), "John Q. Smith");
        assert_eq!(author.firstname(), "John");
        assert_eq!(author.middlename(), Some("Q."));
        assert_eq!(author.lastname(), Some("Smith"));
        assert_eq!(author.email(), Some("john@example.com"));
        assert_eq!(author.initials(), "JQS");
    }

    #[test]
    fn authors_is_empty_without_author_info() {
        let doc = Parser::default().parse("= Title\n\nBody.");

        assert!(doc.authors().is_empty());
    }

    #[test]
    fn authorcount_reflects_author_line() {
        // The `authorcount` attribute counts the resolved authors, whether they
        // come from the author line …
        let doc = Parser::default().parse("= Title\nJane Doe; John Smith\n\nBody.");

        assert_eq!(doc.authors().len(), 2);
        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("2")
        );

        // … or from a single `:author:` attribute entry.
        let doc = Parser::default().parse(":author: Jane Doe\n\nBody.");

        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("1")
        );

        // A document with no author information reports a count of zero.
        let doc = Parser::default().parse("= Title\n\nBody.");

        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("0")
        );
    }

    #[test]
    fn explicit_authorinitials_after_author_still_wins() {
        // An explicit `:authorinitials:` entry is honored regardless of whether
        // it precedes or follows the `:author:` entry.
        let doc = Parser::default().parse(":author: Doc Writer\n:authorinitials: DOC\n\nBody.");

        assert_eq!(
            doc.attribute_value("authorinitials"),
            InterpretedValue::Value("DOC")
        );

        // A second `:author:` entry after the explicit initials does not clobber
        // them.
        let doc = Parser::default()
            .parse(":author: Jane Roe\n:authorinitials: DOC\n:author: Doc Writer\n\nBody.");

        assert_eq!(
            doc.attribute_value("author"),
            InterpretedValue::Value("Doc Writer")
        );
        assert_eq!(
            doc.attribute_value("authorinitials"),
            InterpretedValue::Value("DOC")
        );
    }

    #[test]
    fn later_author_entry_redrives_initials_without_explicit_override() {
        // Without an explicit `:authorinitials:` entry, a later `:author:`
        // overwrites the initials derived from the earlier one.
        let doc = Parser::default().parse(":author: Jane Roe\n:author: Doc Writer\n\nBody.");

        assert_eq!(
            doc.attribute_value("authorinitials"),
            InterpretedValue::Value("DW")
        );
    }

    #[test]
    fn explicit_authorinitials_not_preserved_for_indexed_or_authors_forms() {
        // The explicit-`:authorinitials:` override is honored only for a single
        // `:author:` entry. For the indexed `author_N` form (and, as tested
        // elsewhere, the `:authors:` form) the derived initials overwrite it,
        // matching Asciidoctor (see [`set_author_metadata`]).
        let doc = Parser::default().parse(":authorinitials: DOC\n:author_1: Doc Writer\n\nBody.");

        assert_eq!(
            doc.attribute_value("author"),
            InterpretedValue::Value("Doc Writer")
        );
        assert_eq!(
            doc.attribute_value("authorinitials"),
            InterpretedValue::Value("DW")
        );
    }

    #[test]
    fn authors_attribute_splits_into_indexed_authors() {
        // A semicolon-separated `:authors:` entry populates the author list and
        // the derived per-author attributes.
        let doc = Parser::default().parse(":authors: Jane Doe; John Q. Smith\n\nBody.");

        assert_eq!(doc.authors().len(), 2);
        assert_eq!(
            doc.attribute_value("authors"),
            InterpretedValue::Value("Jane Doe, John Q. Smith")
        );
        assert_eq!(
            doc.attribute_value("author"),
            InterpretedValue::Value("Jane Doe")
        );
        assert_eq!(
            doc.attribute_value("author_2"),
            InterpretedValue::Value("John Q. Smith")
        );
        assert_eq!(
            doc.attribute_value("middlename_2"),
            InterpretedValue::Value("Q.")
        );
        assert_eq!(
            doc.attribute_value("authorinitials_2"),
            InterpretedValue::Value("JQS")
        );
    }

    #[test]
    fn authors_attribute_attaches_companion_emails_and_base_middlename() {
        // Companion `:email_N:` entries attach to each split author (`email_1`
        // also fills the base `email`), and the first author's middle name lands
        // on the unsuffixed `middlename`.
        let doc = Parser::default().parse(
            ":authors: Jane Q. Doe; John Smith\n:email_1: jane@example.com\n:email_2: john@example.com\n\nBody.",
        );

        let authors = doc.authors();
        assert_eq!(authors.len(), 2);
        assert_eq!(authors.first().unwrap().email(), Some("jane@example.com"));
        assert_eq!(authors.get(1).unwrap().email(), Some("john@example.com"));

        assert_eq!(
            doc.attribute_value("middlename"),
            InterpretedValue::Value("Q.")
        );
        assert_eq!(
            doc.attribute_value("email"),
            InterpretedValue::Value("jane@example.com")
        );
        assert_eq!(
            doc.attribute_value("email_2"),
            InterpretedValue::Value("john@example.com")
        );
    }

    #[test]
    fn authors_attribute_semicolon_without_space_is_one_author() {
        // A semicolon that is not followed by a space (or the end of the value)
        // does not separate authors.
        let doc = Parser::default().parse(":authors: Joe Doe;Smith Johnson\n\nBody.");

        assert_eq!(doc.authors().len(), 1);
        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("1")
        );
    }

    #[test]
    fn authors_attribute_single_name_authors_and_trailing_separator() {
        // Single-name authors carry no last name, and a trailing `;` (a
        // separator at the end of the value) contributes no extra author.
        let doc = Parser::default().parse(":authors: Cher; Madonna;\n\nBody.");

        assert_eq!(doc.authors().len(), 2);
        assert_eq!(
            doc.attribute_value("authors"),
            InterpretedValue::Value("Cher, Madonna")
        );
        assert_eq!(
            doc.attribute_value("author"),
            InterpretedValue::Value("Cher")
        );
        assert_eq!(doc.attribute_value("lastname"), InterpretedValue::Unset);
        assert_eq!(
            doc.attribute_value("authorinitials"),
            InterpretedValue::Value("C")
        );
        assert_eq!(
            doc.attribute_value("author_2"),
            InterpretedValue::Value("Madonna")
        );
        assert_eq!(doc.attribute_value("lastname_2"), InterpretedValue::Unset);
        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("2")
        );
    }

    #[test]
    fn authors_attribute_with_only_empty_entries_yields_no_authors() {
        // An `:authors:` value that splits into only empty entries resolves to no
        // authors, so the derived attributes stay unset and `authorcount` is 0.
        let doc = Parser::default().parse(":authors: ;\n\nBody.");

        assert!(doc.authors().is_empty());
        assert_eq!(doc.attribute_value("author"), InterpretedValue::Unset);
        assert_eq!(
            doc.attribute_value("authorcount"),
            InterpretedValue::Value("0")
        );

        // With no authors resolved, the raw `authors` value is left as written
        // (never rewritten to a comma-joined list) — matching Asciidoctor, whose
        // `process_authors` returns only `authorcount` for an all-empty value.
        assert_eq!(doc.attribute_value("authors"), InterpretedValue::Value(";"));
    }

    #[test]
    fn author_attribute_takes_precedence_over_authors() {
        // A base `:author:` entry wins over a semicolon-separated `:authors:`
        // entry (matching Asciidoctor's `if author … elsif authors` order), so
        // only the single author is resolved.
        let doc = Parser::default()
            .parse(":author: Solo Writer\n:authors: Jane Doe; John Smith\n\nBody.");

        assert_eq!(doc.authors().len(), 1);
        assert_eq!(
            doc.attribute_value("author"),
            InterpretedValue::Value("Solo Writer")
        );
        assert_eq!(doc.attribute_value("author_2"), InterpretedValue::Unset);
    }

    #[test]
    fn author_unset_after_being_assigned_yields_no_authors() {
        // A later `:author!:` unsets the attribute; the resolved author list
        // must reflect the removal, not the earlier assignment.
        let doc = Parser::default().parse("= Title\n:author: Jane Doe\n:author!:\n\nBody.");

        assert_eq!(doc.attribute_value("author"), InterpretedValue::Unset);
        assert!(doc.authors().is_empty());
    }

    #[test]
    fn impl_debug() {
        let doc = Parser::default().parse("= Example Title\n\nabc\n\ndef");
        let header = doc.header();

        assert_eq!(
            format!("{header:#?}"),
            r#"Header {
    title_source: Some(
        Span {
            data: "Example Title",
            line: 1,
            col: 3,
            offset: 2,
        },
    ),
    title: Some(
        "Example Title",
    ),
    doctitle: Some(
        "Example Title",
    ),
    main_title: Some(
        "Example Title",
    ),
    subtitle: None,
    id: None,
    roles: [],
    attributes: &[],
    author_line: None,
    authors: [],
    revision_line: None,
    comments: &[],
    source: Span {
        data: "= Example Title",
        line: 1,
        col: 1,
        offset: 0,
    },
}"#
        );
    }

    #[test]
    fn no_subtitle() {
        // A title without a colon-space sequence has no subtitle, and its main
        // title equals its full title.
        let doc = Parser::default().parse("= Just the Title");
        let header = doc.header();

        assert_eq!(header.title(), Some("Just the Title"));
        assert_eq!(header.main_title(), Some("Just the Title"));
        assert_eq!(header.subtitle(), None);
    }

    #[test]
    fn no_title() {
        // With no document title at all, every title accessor returns `None`.
        let doc = Parser::default().parse(":foo: bar\n\nbody");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.main_title(), None);
        assert_eq!(header.subtitle(), None);
    }

    #[test]
    fn colon_without_space_is_not_a_separator() {
        // The separator is a colon *followed by a space*; a bare colon does not
        // partition the title.
        let doc = Parser::default().parse("= Ratio 3:1 Explained");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Ratio 3:1 Explained"));
        assert_eq!(header.subtitle(), None);
    }

    #[test]
    fn subtitle_available_on_document() {
        // The subtitle is reachable directly from `Document` as well as from its
        // `Header`.
        let doc = Parser::default().parse("= Main Title: Subtitle");

        assert_eq!(doc.doctitle(), Some("Main Title: Subtitle"));
        assert_eq!(doc.subtitle(), Some("Subtitle"));
    }

    #[test]
    fn separator_block_attribute_above_title() {
        // A `[separator=::]` block attribute above the title changes the
        // subtitle separator for that title.
        let doc = Parser::default().parse("[separator=::]\n= Main Title:: Subtitle");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Main Title"));
        assert_eq!(header.subtitle(), Some("Subtitle"));

        // The custom separator replaces the default: a plain colon-space no
        // longer partitions the title.
        let doc = Parser::default().parse("[separator=::]\n= Main: Title:: Subtitle");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Main: Title"));
        assert_eq!(header.subtitle(), Some("Subtitle"));
    }

    #[test]
    fn separator_attribute_entry_overrides_block_attribute() {
        // When both are present, the later assignment wins in document order.
        // Here the `:title-separator:` entry follows the block attribute.
        let doc = Parser::default()
            .parse("[separator=::]\n= Main Title;; Subtitle\n:title-separator: ;;");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Main Title"));
        assert_eq!(header.subtitle(), Some("Subtitle"));
    }

    #[test]
    fn unrecognized_block_attribute_above_title_is_consumed() {
        // A well-formed block attribute line above the document title is now
        // parsed as document metadata, so the title that follows is recognized
        // even when the line carries no attribute this crate folds. An
        // unrecognized attribute (here `foo`) simply contributes no document
        // metadata rather than terminating the header.
        let doc = Parser::default().parse("[foo=bar]\n= A Header Title");
        let header = doc.header();

        assert_eq!(header.title(), Some("A Header Title"));
        assert_eq!(header.subtitle(), None);
        assert_eq!(doc.attribute_value("foo"), InterpretedValue::Unset);
    }

    #[test]
    fn reftext_block_attribute_above_title() {
        // A `[reftext="…"]` block attribute above the title recovers the title
        // (previously lost) and folds the value into the document's `reftext`
        // attribute, matching the `:reftext:` header attribute.
        let doc =
            Parser::default().parse("[reftext=\"Links and Stuff\"]\n= Links & Stuff\n\nBody.");
        let header = doc.header();

        assert_eq!(header.title(), Some("Links &amp; Stuff"));
        assert_eq!(
            doc.attribute_value("reftext"),
            InterpretedValue::Value("Links and Stuff")
        );
        assert_eq!(rendered_paragraphs(&doc), vec!["Body."]);
    }

    #[test]
    fn id_block_attribute_above_title() {
        // The `[#id]` shorthand above the title assigns the document ID and
        // still recovers the title.
        let doc = Parser::default().parse("[#docid]\n= Document Title\n\nBody.");
        let header = doc.header();

        assert_eq!(header.title(), Some("Document Title"));
        assert_eq!(header.id(), Some("docid"));
        assert_eq!(doc.id(), Some("docid"));

        // The longhand `[id=…]` form is equivalent.
        let doc = Parser::default().parse("[id=docid]\n= Document Title");
        assert_eq!(doc.header().id(), Some("docid"));
    }

    #[test]
    fn stacked_block_attributes_above_title() {
        // Multiple block attribute lines may stack above the document title;
        // each folds into the document's metadata and the title is still
        // recovered (see #821). Here an `[#id]` line and a `[reftext="…"]` line
        // both sit above the title.
        let doc = Parser::default()
            .parse("[#docid]\n[reftext=\"Links and Stuff\"]\n= Links & Stuff\n\nBody.");
        let header = doc.header();

        assert_eq!(header.title(), Some("Links &amp; Stuff"));
        assert_eq!(header.id(), Some("docid"));
        assert_eq!(doc.id(), Some("docid"));
        assert_eq!(
            doc.attribute_value("reftext"),
            InterpretedValue::Value("Links and Stuff")
        );
        assert_eq!(rendered_paragraphs(&doc), vec!["Body."]);
    }

    #[test]
    fn stacked_block_attributes_combine_roles() {
        // Roles from stacked block attribute lines all fold into the document's
        // `role` attribute (space-joined) and are surfaced through the block
        // API, alongside an ID set on a separate line.
        let doc = Parser::default().parse("[#docid]\n[.one]\n[.two]\n= Document Title");
        let header = doc.header();

        assert_eq!(header.title(), Some("Document Title"));
        assert_eq!(header.id(), Some("docid"));
        assert_eq!(
            doc.attribute_value("role"),
            InterpretedValue::Value("one two")
        );
        assert_eq!(header.roles(), vec!["one", "two"]);
    }

    #[test]
    fn stacked_block_attributes_require_a_following_title() {
        // Stacked block attribute lines are only folded when a document title
        // eventually follows. Without one, the run is left for the block parser
        // and no title is recognized.
        let doc = Parser::default().parse("[#docid]\n[reftext=\"Stuff\"]\n\nBody.");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.id(), None);
        assert_eq!(doc.attribute_value("reftext"), InterpretedValue::Unset);
    }

    #[test]
    fn stacked_block_attributes_stop_at_a_block_anchor() {
        // A `[[anchor]]` line breaks the run of foldable metadata lines: the
        // scan stops there without seeing the title, so nothing above it is
        // folded and the header terminates (matching the single-line anchor
        // rejection).
        let doc = Parser::default().parse("[#docid]\n[[anchor]]\n= Some Title");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.id(), None);
    }

    #[test]
    fn role_block_attribute_above_title() {
        // A `[role=…]` block attribute above the title folds into the document's
        // `role` attribute; multiple roles are space-joined. The same role(s)
        // are surfaced through the block API, so `Header::roles()` and
        // `Document::roles()` agree with the document attribute (see #820).
        let doc = Parser::default().parse("[role=special]\n= Document Title\n\nBody.");
        let header = doc.header();

        assert_eq!(header.title(), Some("Document Title"));
        assert_eq!(
            doc.attribute_value("role"),
            InterpretedValue::Value("special")
        );
        assert_eq!(header.roles(), vec!["special"]);
        assert_eq!(doc.roles(), vec!["special"]);

        // The dot shorthand assigns roles too, and they combine.
        let doc = Parser::default().parse("[.one.two]\n= Document Title");
        assert_eq!(
            doc.attribute_value("role"),
            InterpretedValue::Value("one two")
        );
        assert_eq!(doc.header().roles(), vec!["one", "two"]);
        assert_eq!(doc.roles(), vec!["one", "two"]);
    }

    #[test]
    fn roles_empty_without_block_attribute() {
        // With no role assigned above the title, both the block accessor and the
        // header accessor report no roles.
        let doc = Parser::default().parse("= Document Title\n\nBody.");

        assert!(doc.header().roles().is_empty());
        assert!(doc.roles().is_empty());
    }

    #[test]
    fn options_block_attribute_above_title() {
        // A `[opts=…]` block attribute above the title sets a `<name>-option`
        // document attribute for each option.
        let doc = Parser::default().parse("[opts=\"noheader,autowidth\"]\n= Document Title");

        assert!(doc.is_attribute_set("noheader-option"));
        assert!(doc.is_attribute_set("autowidth-option"));

        // The `%` shorthand is equivalent.
        let doc = Parser::default().parse("[%hardbreaks]\n= Document Title");
        assert!(doc.is_attribute_set("hardbreaks-option"));
    }

    #[test]
    fn bracketed_line_that_is_not_a_separator_attribute_list() {
        // A `[...]` line above the title that isn't a well-formed block
        // attribute list carrying `separator` is not consumed as a separator. A
        // block anchor (`[[...]]`) and a leading-space form are both rejected,
        // so the line terminates the header exactly as any other unrecognized
        // line would.
        let doc = Parser::default().parse("[[anchor]]\n= Some Title: Subtitle");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.subtitle(), None);

        let doc = Parser::default().parse("[ separator=::]\n= Main Title:: Subtitle");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.subtitle(), None);
    }

    #[test]
    fn empty_title_separator_falls_back_to_default() {
        // An explicitly empty `title-separator` falls back to the default
        // `:{sp}` separator rather than partitioning on an empty string.
        let doc = Parser::default().parse("= Main Title: Subtitle\n:title-separator:");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Main Title"));
        assert_eq!(header.subtitle(), Some("Subtitle"));
    }

    #[test]
    fn counter_does_not_shadow_title_separator() {
        // A counter that happens to be named `title-separator` must not be
        // mistaken for the configured separator: partitioning reads the document
        // attribute directly, ignoring the counter overlay. Here the title
        // creates such a counter, but the default `:{sp}` separator still
        // applies.
        let doc = Parser::default().parse("= Main Title: Subtitle {counter:title-separator}");
        let header = doc.header();

        assert_eq!(header.main_title(), Some("Main Title"));
        assert_eq!(header.subtitle(), Some("Subtitle 1"));
    }

    #[test]
    fn skips_block_comment_before_author() {
        // A `////` block comment ahead of the author line is skipped and
        // retained as a single header comment; the author line that follows it
        // is parsed normally.
        let doc = Parser::default()
            .parse("= Title\n////\nAsciidoctor\nrelease artist\n////\nRyan Waldron");
        let header = doc.header();

        let author = header.authors().first().unwrap();
        assert_eq!(author.name(), "Ryan Waldron");

        assert_eq!(header.comments().count(), 1);
        assert_eq!(
            header.comments().next().unwrap().data(),
            "////\nAsciidoctor\nrelease artist\n////"
        );
    }

    #[test]
    fn skips_block_comment_with_blank_lines() {
        // Blank lines inside a header block comment do not terminate the header;
        // the whole block is skipped and the author line is still recognized.
        let doc = Parser::default().parse("= Title\n////\n\nAsciidoctor\n\n////\nRyan Waldron");
        let header = doc.header();

        assert_eq!(header.authors().first().unwrap().name(), "Ryan Waldron");
        assert_eq!(header.comments().count(), 1);
    }

    #[test]
    fn unterminated_block_comment_consumes_rest_of_header() {
        // An unterminated `////` block comment runs to the end of the input,
        // mirroring Asciidoctor; nothing after it is parsed as an author line.
        let doc = Parser::default().parse("= Title\n////\nAsciidoctor\nRyan Waldron");
        let header = doc.header();

        assert!(header.authors().is_empty());
        assert_eq!(header.comments().count(), 1);
    }

    #[test]
    fn longer_block_comment_delimiter_requires_matching_close() {
        // The closing delimiter must repeat the opening line exactly: a `////`
        // line does not close a `/////` block, so the block runs on until the
        // matching `/////` and the author line after it is recognized.
        let doc = Parser::default()
            .parse("= Title\n/////\nAsciidoctor\n////\nstill comment\n/////\nRyan Waldron");
        let header = doc.header();

        assert_eq!(header.authors().first().unwrap().name(), "Ryan Waldron");
        assert_eq!(header.comments().count(), 1);
    }

    #[test]
    fn three_slashes_is_not_a_block_comment() {
        // A line of exactly three slashes is not a block comment delimiter
        // (which requires four or more slashes), so it is not skipped: were it
        // mistaken for an unterminated block comment it would swallow the rest
        // of the header, but instead the author and revision lines before it
        // are captured as before.
        let mut parser = Parser::default();
        parser.parse("= Title\nJoe Cool\nv1.0\n///\nstuff");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Joe Cool")
        );
        assert_eq!(
            parser.attribute_value("revnumber"),
            InterpretedValue::Value("1.0")
        );
    }

    mod markdown_style_document_title {
        use crate::tests::prelude::*;

        #[test]
        fn hash_marker_is_a_document_title() {
            let mut parser = Parser::default();
            let mi =
                crate::document::Header::parse(crate::Span::new("# Just the Title"), &mut parser)
                    .unwrap_if_no_warnings();

            assert_eq!(
                mi.item,
                Header {
                    title_source: Some(Span {
                        data: "Just the Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Just the Title"),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "# Just the Title",
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
                    col: 17,
                    offset: 16
                }
            );
        }

        #[test]
        fn sets_doctitle_attribute() {
            let doc = Parser::default().parse("# Doc Title\n\n{doctitle}");
            assert_eq!(doc.header().title(), Some("Doc Title"));
            assert_eq!(rendered_paragraphs(&doc), vec!["Doc Title"]);
        }

        #[test]
        fn strips_symmetric_close() {
            let doc = Parser::default().parse("# Doc Title #");
            assert_eq!(doc.header().title(), Some("Doc Title"));
        }

        #[test]
        fn does_not_strip_mismatched_close() {
            // The close must repeat the opening marker, so a trailing `=` after
            // a `#` title is title text.
            let doc = Parser::default().parse("# Doc Title =");
            assert_eq!(doc.header().title(), Some("Doc Title ="));
        }

        #[test]
        fn requires_whitespace_after_marker() {
            let doc = Parser::default().parse("#Doc Title");

            assert_eq!(doc.header().title(), None);
            assert_eq!(rendered_paragraphs(&doc), vec!["#Doc Title"]);
        }

        #[test]
        fn carries_the_rest_of_the_header() {
            // Everything that may follow an `=` title — attribute entries, the
            // author line, the revision line — follows a `#` title too.
            let doc = Parser::default()
                .parse("# Doc Title\n:foo: bar\nKismet R. Lee <kismet@asciidoctor.org>\nv1.0\n");
            let header = doc.header();

            assert_eq!(header.title(), Some("Doc Title"));
            assert_eq!(header.authors().first().unwrap().firstname(), "Kismet");
            assert_eq!(header.revision_line().unwrap().revnumber().unwrap(), "1.0");
        }

        #[test]
        fn partitions_subtitle() {
            let doc = Parser::default().parse("# Main Title: Subtitle");
            let header = doc.header();

            assert_eq!(header.main_title(), Some("Main Title"));
            assert_eq!(header.subtitle(), Some("Subtitle"));
        }

        #[test]
        fn separator_block_attribute_above_title() {
            // The `[separator=…]` line is only intercepted when a document title
            // follows it; a `#` title qualifies just as an `=` title does.
            let doc = Parser::default().parse("[separator=::]\n# Main Title:: Subtitle");
            let header = doc.header();

            assert_eq!(header.main_title(), Some("Main Title"));
            assert_eq!(header.subtitle(), Some("Subtitle"));
        }

        #[test]
        fn markdown_title_followed_by_markdown_sections() {
            let doc = Parser::default().parse("# Doc Title\n\n## Section One\n\nblah blah\n");

            assert_eq!(doc.header().title(), Some("Doc Title"));

            let section = first_section(&doc);

            assert_eq!(section.level(), 1);
            assert_eq!(section.section_title(), "Section One");
        }
    }
}
