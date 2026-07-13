use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, SubstitutionGroup},
    document::{Attribute, Author, AuthorLine, InterpretedValue, RevisionLine},
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
    main_title: Option<String>,
    subtitle: Option<String>,
    attributes: Vec<Attribute<'src>>,
    author_line: Option<AuthorLine<'src>>,
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
        let mut attributes: Vec<Attribute> = vec![];
        let mut author_line: Option<AuthorLine<'src>> = None;
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
            } else if line.starts_with(':')
                && let Some(attr) = Attribute::parse(source, parser)
            {
                // Special handling for :author: attribute to populate individual author
                // attributes.
                if attr.item.name().data().eq_ignore_ascii_case("author")
                    && let Some(raw_value) = attr.item.raw_value()
                    && let Some(author) = Author::parse(raw_value.data(), parser)
                {
                    // Set individual author attributes.
                    parser.set_attribute_by_value_from_header("firstname", author.firstname());
                    if let Some(middlename) = author.middlename() {
                        parser.set_attribute_by_value_from_header("middlename", middlename);
                    }
                    if let Some(lastname) = author.lastname() {
                        parser.set_attribute_by_value_from_header("lastname", lastname);
                    }
                    parser.set_attribute_by_value_from_header("authorinitials", author.initials());
                    if let Some(email) = author.email() {
                        parser.set_attribute_by_value_from_header("email", email);
                    }
                }

                parser.set_attribute_from_header(&attr.item, &mut warnings);
                attributes.push(attr.item);
                source = attr.after;
            } else if title.is_none()
                && line.starts_with('[')
                && line.ends_with(']')
                && line_mi.after.take_normalized_line().item.starts_with("= ")
                && let Some((separator, separator_warnings)) =
                    parse_separator_attribute(line, parser)
            {
                warnings.extend(separator_warnings);
                // A `separator` block attribute directly above the document title
                // sets the subtitle separator. It behaves exactly like assigning
                // the `title-separator` document attribute at this point in the
                // header, so both mechanisms share the same partitioning logic
                // and follow document order when both are present.
                //
                // The line is only intercepted when a document title immediately
                // follows; otherwise it is block metadata for the body (e.g. a
                // table's `separator`) and is left for the block parser.
                parser.set_attribute_by_value_from_header("title-separator", separator);
                source = line_mi.after;
            } else if title.is_none() && line.starts_with("= ") {
                let title_span = line.discard(2).discard_whitespace();
                let title_str = apply_header_subs(title_span.data(), parser);

                parser.set_attribute_by_value_from_header("doctitle", &title_str);

                title = Some(title_str);
                title_source = Some(title_span);
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

        MatchAndWarnings {
            item: MatchedItem {
                item: Self {
                    title_source,
                    title,
                    main_title,
                    subtitle,
                    attributes,
                    author_line,
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

    /// Return an iterator over the attributes in this header.
    pub fn attributes(&'src self) -> Iter<'src, Attribute<'src>> {
        self.attributes.iter()
    }

    /// Returns the author line, if found.
    pub fn author_line(&self) -> Option<&AuthorLine<'src>> {
        self.author_line.as_ref()
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

/// Extract the value of a `separator` attribute from a block attribute line
/// (e.g. `[separator=::]`) appearing above the document title.
///
/// The `line` is expected to begin with `[` and end with `]`. Returns the
/// `separator` value together with any warnings raised while parsing the
/// attribute list if the line is a well-formed block attribute list that
/// contains a `separator`, and `None` otherwise (so the caller can fall through
/// to its normal handling of the line). The warnings are only surfaced when the
/// line is actually consumed as a separator; otherwise the line is left for the
/// block parser, which reports them on its own path.
fn parse_separator_attribute<'src>(
    line: Span<'src>,
    parser: &Parser,
) -> Option<(String, Vec<Warning<'src>>)> {
    // Drop the enclosing square brackets now that the caller has confirmed they
    // are present.
    let inner = line.slice(1..line.len() - 1);

    // Reject forms that are not block attribute lists, mirroring the checks used
    // when parsing block metadata elsewhere: a leading space or tab, an empty
    // list, or a `[[anchor]]` block anchor.
    if inner.is_empty()
        || inner.starts_with(' ')
        || inner.starts_with('\t')
        || (inner.starts_with('[') && inner.ends_with(']'))
    {
        return None;
    }

    let MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: _,
        },
        warnings,
    } = Attrlist::parse(inner, parser, AttrlistContext::Block);

    let separator = attrlist
        .named_attribute("separator")
        .map(|attr| attr.value().to_string())?;

    Some((separator, warnings))
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
            .field("main_title", &self.main_title)
            .field("subtitle", &self.subtitle)
            .field("attributes", &DebugSliceReference(&self.attributes))
            .field("author_line", &self.author_line)
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
    main_title: Some(
        "Example Title",
    ),
    subtitle: None,
    attributes: &[],
    author_line: None,
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
    fn non_separator_block_attribute_terminates_header() {
        // A block attribute line above the title that isn't a `separator`
        // terminates the header without a title, preserving prior behavior.
        let doc = Parser::default().parse("[foo=bar]\n= Not A Header Title");
        let header = doc.header();

        assert_eq!(header.title(), None);
        assert_eq!(header.subtitle(), None);
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
}
