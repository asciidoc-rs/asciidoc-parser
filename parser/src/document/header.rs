use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    content::{Content, SubstitutionGroup},
    document::{Attribute, Author, AuthorLine, RevisionLine},
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
                    });
                }
                break;
            }
        }

        let after = source.discard_empty_lines();
        let source = original_source.trim_remainder(source);

        MatchAndWarnings {
            item: MatchedItem {
                item: Self {
                    title_source,
                    title,
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
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
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
}
