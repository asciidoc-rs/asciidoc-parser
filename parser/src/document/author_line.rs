use std::slice::Iter;

use crate::{HasSpan, Parser, Span, document::Author};

/// The author line is directly after the document title line in the document
/// header. When the content on this line is structured correctly, the processor
/// assigns the content to the built-in author and email attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorLine<'src> {
    authors: Vec<Author>,
    source: Span<'src>,
}

impl<'src> AuthorLine<'src> {
    pub(crate) fn parse(source: Span<'src>, parser: &mut Parser) -> Self {
        let authors: Vec<Author> = source
            .data()
            .split("; ")
            .filter_map(|raw_author| Author::parse(raw_author, parser))
            .collect();

        for (index, author) in authors.iter().enumerate() {
            set_nth_attribute(parser, "author", index, author.name());
            set_nth_attribute(parser, "authorinitials", index, author.initials());
            set_nth_attribute(parser, "firstname", index, author.firstname());

            if let Some(middlename) = author.middlename() {
                set_nth_attribute(parser, "middlename", index, middlename);
            }

            if let Some(lastname) = author.lastname() {
                set_nth_attribute(parser, "lastname", index, lastname);
            }

            if let Some(email) = author.email() {
                set_nth_attribute(parser, "email", index, email);
            }
        }

        Self { authors, source }
    }

    /// Return an iterator over the authors in this author line.
    pub fn authors(&'src self) -> Iter<'src, Author> {
        self.authors.iter()
    }
}

fn set_nth_attribute<V: AsRef<str>>(parser: &mut Parser, name: &str, index: usize, value: V) {
    let name = if index == 0 {
        name.to_string()
    } else {
        format!("{name}_{count}", count = index + 1)
    };

    parser.set_attribute_by_value_from_header(name, value);
}

impl<'src> HasSpan<'src> for AuthorLine<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use crate::{Parser, parser::ModificationContext, tests::prelude::*};

    #[test]
    fn empty_line() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(crate::Span::new(""), &mut parser);

        assert_eq!(
            &al,
            AuthorLine {
                authors: &[],
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn attr_sub_with_html_encoding_fallback() {
        // Test case for code coverage: input contains attributes but after expansion
        // doesn't match AUTHOR regex and contains angle brackets that need HTML
        // encoding.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "weird-content",
            "Complex <weird> & stuff",
            ModificationContext::Anywhere,
        );

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Some {weird-content} pattern"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Some Complex &lt;weird&gt; &amp; stuff pattern",
                    firstname: "Some Complex &lt;weird&gt; &amp; stuff pattern",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "Some {weird-content} pattern",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn empty_author() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Author One; ; Author Three"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[
                    Author {
                        name: "Author One",
                        firstname: "Author",
                        middlename: None,
                        lastname: Some("One",),
                        email: None,
                    },
                    Author {
                        name: "Author Three",
                        firstname: "Author",
                        middlename: None,
                        lastname: Some("Three",),
                        email: None,
                    },
                ],
                source: Span {
                    data: "Author One; ; Author Three",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn one_simple_author() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Kismet R. Lee <kismet@asciidoctor.org>"),
            &mut parser,
        );

        assert_eq!(
            &al,
            AuthorLine {
                authors: &[Author {
                    name: "Kismet R. Lee",
                    firstname: "Kismet",
                    middlename: Some("R.",),
                    lastname: Some("Lee",),
                    email: Some("kismet@asciidoctor.org",),
                },],
                source: Span {
                    data: "Kismet R. Lee <kismet@asciidoctor.org>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn author_without_middle_name() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Doc Writer <doc@example.com>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Doc Writer",
                    firstname: "Doc",
                    middlename: None,
                    lastname: Some("Writer",),
                    email: Some("doc@example.com",),
                },],
                source: Span {
                    data: "Doc Writer <doc@example.com>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn too_many_names() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Four Names Not Supported <doc@example.com>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Four Names Not Supported &lt;doc@example.com&gt;",
                    firstname: "Four Names Not Supported &lt;doc@example.com&gt;",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "Four Names Not Supported <doc@example.com>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn one_name() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("John <john@example.com>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "John",
                    firstname: "John",
                    middlename: None,
                    lastname: None,
                    email: Some("john@example.com",),
                },],
                source: Span {
                    data: "John <john@example.com>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn underscore_join() {
        let mut parser = Parser::default();

        let al =
            crate::document::AuthorLine::parse(crate::Span::new("Mary_Sue Brontë"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Mary Sue Brontë", // Underscore replaced with space
                    firstname: "Mary Sue",   // Underscore replaced with space
                    middlename: None,
                    lastname: Some("Brontë",),
                    email: None,
                },],
                source: Span {
                    data: "Mary_Sue Brontë",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn greek() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Αλέξανδρος Παπαδόπουλος"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Αλέξανδρος Παπαδόπουλος",
                    firstname: "Αλέξανδρος",
                    middlename: None,
                    lastname: Some("Παπαδόπουλος",),
                    email: None,
                },],
                source: Span {
                    data: "Αλέξανδρος Παπαδόπουλος",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn japanese() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(crate::Span::new("山田太郎"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "山田太郎",
                    firstname: "山田太郎",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "山田太郎",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn arabic() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(crate::Span::new("عبد_الله"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "عبد الله",      // Underscore replaced with space
                    firstname: "عبد الله", // Underscore replaced with space
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "عبد_الله",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn underscore_replacement_in_all_name_parts() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("John_Paul Mary_Jane Smith_Jones <email@example.com>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "John Paul Mary Jane Smith Jones", // Underscore replaced with space
                    firstname: "John Paul",                  // Underscore replaced with space
                    middlename: Some("Mary Jane"),           // Underscore replaced with space
                    lastname: Some("Smith Jones"),           // Underscore replaced with space
                    email: Some("email@example.com"),
                },],
                source: Span {
                    data: "John_Paul Mary_Jane Smith_Jones <email@example.com>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn multiple_underscores_in_name_parts() {
        let mut parser = Parser::default();

        let al =
            crate::document::AuthorLine::parse(crate::Span::new("A_B_C D_E_F G_H_I"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "A B C D E F G H I", // Multiple underscores replaced with spaces
                    firstname: "A B C",        // Multiple underscores replaced with spaces
                    middlename: Some("D E F"), // Multiple underscores replaced with spaces
                    lastname: Some("G H I"),   // Multiple underscores replaced with spaces
                    email: None,
                },],
                source: Span {
                    data: "A_B_C D_E_F G_H_I",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn underscore_replacement_with_attribute_substitution() {
        let mut parser = Parser::default()
            .with_intrinsic_attribute("first-part", "John_Paul", ModificationContext::Anywhere)
            .with_intrinsic_attribute("last-part", "Smith_Jones", ModificationContext::Anywhere);

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("{first-part} {last-part} <email@example.com>"),
            &mut parser,
        );

        // Note: This test documents the current behavior where attribute substitution
        // happens after parsing, which results in HTML encoding of the angle brackets.
        // The underscore replacement should still work on the attribute-substituted
        // values.
        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "John Paul Smith Jones &lt;email@example.com&gt;", /* Underscore
                                                                              * replaced
                                                                              * with space */
                    firstname: "John Paul Smith Jones &lt;email@example.com&gt;", /* Underscore
                                                                                   * replaced with
                                                                                   * space */
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "{first-part} {last-part} <email@example.com>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn attr_sub_email() {
        let mut parser = Parser::default()
            .with_intrinsic_attribute(
                "jane-email",
                "jane@example.com",
                ModificationContext::Anywhere,
            )
            .with_intrinsic_attribute(
                "john-email",
                "john@example.com",
                ModificationContext::Anywhere,
            );

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("Jane Smith <{jane-email}>; John Doe <{john-email}>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[
                    Author {
                        name: "Jane Smith",
                        firstname: "Jane",
                        middlename: None,
                        lastname: Some("Smith",),
                        email: Some("jane@example.com",),
                    },
                    Author {
                        name: "John Doe",
                        firstname: "John",
                        middlename: None,
                        lastname: Some("Doe",),
                        email: Some("john@example.com",),
                    },
                ],
                source: Span {
                    data: "Jane Smith <{jane-email}>; John Doe <{john-email}>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn attr_sub_applied_after_parsing() {
        // This is to demonstrate compatibility with Ruby asciidoctor behavior. In that
        // implementation, the attribute substitution is applied *after* parsing for
        // individual authors, which results in the unexpected treatment that the entire
        // list is one author with mangled results.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "author-list",
            "Jane Smith <jane@example.com>; John Doe <john@example.com>",
            ModificationContext::Anywhere,
        );

        let al = crate::document::AuthorLine::parse(crate::Span::new("{author-list}"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Jane Smith <jane@example.com>; John Doe <john@example.com>",
                    firstname: "Jane Smith <jane@example.com>; John Doe <john@example.com>",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "{author-list}",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn attr_sub_for_individual_author() {
        let mut parser = Parser::default().with_intrinsic_attribute(
            "full-author",
            "John Doe <john@example.com>",
            ModificationContext::Anywhere,
        );

        let al = crate::document::AuthorLine::parse(crate::Span::new("{full-author}"), &mut parser);

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "John Doe <john@example.com>",
                    firstname: "John Doe <john@example.com>",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "{full-author}",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn err_individual_name_components_as_attributes() {
        // This approach doesn't work in Ruby AsciiDoctor either.
        let mut parser = Parser::default()
            .with_intrinsic_attribute("first-name", "Jane", ModificationContext::Anywhere)
            .with_intrinsic_attribute("last-name", "Smith", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "author-email",
                "jane@example.com",
                ModificationContext::Anywhere,
            );

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("{first-name} {last-name} <{author-email}>"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[Author {
                    name: "Jane Smith &lt;jane@example.com&gt;",
                    firstname: "Jane Smith &lt;jane@example.com&gt;",
                    middlename: None,
                    lastname: None,
                    email: None,
                },],
                source: Span {
                    data: "{first-name} {last-name} <{author-email}>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn sets_author_attributes_single_author_with_all_parts() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title\nKismet R. Lee <kismet@asciidoctor.org>");

        // Primary author attributes
        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Kismet R. Lee")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Kismet")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("R.")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Lee")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("KRL")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("kismet@asciidoctor.org")
        );
    }

    #[test]
    fn sets_author_attributes_single_author_without_middle_name() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title\nDoc Writer <doc@example.com>");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Doc Writer")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Doc")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Writer")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("DW")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("doc@example.com")
        );
    }

    #[test]
    fn sets_author_attributes_single_author_first_name_only() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title\nJohn <john@example.com>");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("John")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("John")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(parser.attribute_value("lastname"), InterpretedValue::Unset);
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("J")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("john@example.com")
        );
    }

    #[test]
    fn sets_author_attributes_single_author_without_email() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title\nMary Sue Brontë");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Mary Sue Brontë")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Mary")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Sue")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Brontë")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("MSB")
        );
        assert_eq!(parser.attribute_value("email"), InterpretedValue::Unset);
    }

    #[test]
    fn sets_author_attributes_multiple_authors() {
        let mut parser = Parser::default();
        let _doc = parser
            .parse("= Document Title\nJane Smith <jane@example.com>; John Doe <john@example.com>");

        // First author (primary)
        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Jane Smith")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Jane")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Smith")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("JS")
        );
        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("jane@example.com")
        );

        // Second author
        assert_eq!(
            parser.attribute_value("author_2"),
            InterpretedValue::Value("John Doe")
        );
        assert_eq!(
            parser.attribute_value("firstname_2"),
            InterpretedValue::Value("John")
        );
        assert_eq!(
            parser.attribute_value("lastname_2"),
            InterpretedValue::Value("Doe")
        );
        assert_eq!(
            parser.attribute_value("authorinitials_2"),
            InterpretedValue::Value("JD")
        );
        assert_eq!(
            parser.attribute_value("email_2"),
            InterpretedValue::Value("john@example.com")
        );

        // Verify middlename attributes are unset for both authors
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Unset
        );
        assert_eq!(
            parser.attribute_value("middlename_2"),
            InterpretedValue::Unset
        );
    }

    #[test]
    fn sets_author_attributes_unicode_names() {
        let mut parser = Parser::default();
        let _doc = parser.parse("= Document Title\nΑλέξανδρος Μ. Παπαδόπουλος");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("Αλέξανδρος Μ. Παπαδόπουλος")
        );
        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("Αλέξανδρος")
        );
        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Μ.")
        );
        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Παπαδόπουλος")
        );
        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("ΑΜΠ")
        );
    }

    #[test]
    fn semicolon_in_character_reference_not_treated_as_separator() {
        let mut parser = Parser::default();

        let al = crate::document::AuthorLine::parse(
            crate::Span::new("AsciiDoc&#174;{empty} WG; Another Author"),
            &mut parser,
        );

        assert_eq!(
            al,
            AuthorLine {
                authors: &[
                    Author {
                        name: "AsciiDoc&#174; WG",
                        firstname: "AsciiDoc&#174;",
                        middlename: None,
                        lastname: Some("WG"),
                        email: None,
                    },
                    Author {
                        name: "Another Author",
                        firstname: "Another",
                        middlename: None,
                        lastname: Some("Author"),
                        email: None,
                    },
                ],
                source: Span {
                    data: "AsciiDoc&#174;{empty} WG; Another Author",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );
    }

    #[test]
    fn comprehensive_author_attribute_test() {
        // This test verifies that all author attribute types work correctly for
        // multiple authors, including edge cases like missing middle names and emails.

        let mut parser = Parser::default();
        let doc = parser.parse("= Document Title\nFirst Second Last <first@example.com>; Only First; A B C <abc@example.com>; No Email Guy");

        assert_eq!(
            parser.attribute_value("author"),
            InterpretedValue::Value("First Second Last")
        );

        assert_eq!(
            parser.attribute_value("firstname"),
            InterpretedValue::Value("First")
        );

        assert_eq!(
            parser.attribute_value("middlename"),
            InterpretedValue::Value("Second")
        );

        assert_eq!(
            parser.attribute_value("lastname"),
            InterpretedValue::Value("Last")
        );

        assert_eq!(
            parser.attribute_value("authorinitials"),
            InterpretedValue::Value("FSL")
        );

        assert_eq!(
            parser.attribute_value("email"),
            InterpretedValue::Value("first@example.com")
        );

        assert_eq!(
            parser.attribute_value("author_2"),
            InterpretedValue::Value("Only First")
        );

        assert_eq!(
            parser.attribute_value("firstname_2"),
            InterpretedValue::Value("Only")
        );

        assert_eq!(
            parser.attribute_value("middlename_2"),
            InterpretedValue::Unset
        );

        assert_eq!(
            parser.attribute_value("lastname_2"),
            InterpretedValue::Value("First")
        );

        assert_eq!(
            parser.attribute_value("authorinitials_2"),
            InterpretedValue::Value("OF")
        );

        assert_eq!(parser.attribute_value("email_2"), InterpretedValue::Unset);

        assert_eq!(
            parser.attribute_value("author_3"),
            InterpretedValue::Value("A B C")
        );

        assert_eq!(
            parser.attribute_value("firstname_3"),
            InterpretedValue::Value("A")
        );

        assert_eq!(
            parser.attribute_value("middlename_3"),
            InterpretedValue::Value("B")
        );

        assert_eq!(
            parser.attribute_value("lastname_3"),
            InterpretedValue::Value("C")
        );

        assert_eq!(
            parser.attribute_value("authorinitials_3"),
            InterpretedValue::Value("ABC")
        );

        assert_eq!(
            parser.attribute_value("email_3"),
            InterpretedValue::Value("abc@example.com")
        );

        assert_eq!(
            parser.attribute_value("author_4"),
            InterpretedValue::Value("No Email Guy")
        );

        assert_eq!(
            parser.attribute_value("firstname_4"),
            InterpretedValue::Value("No")
        );

        assert_eq!(
            parser.attribute_value("middlename_4"),
            InterpretedValue::Value("Email")
        );

        assert_eq!(
            parser.attribute_value("lastname_4"),
            InterpretedValue::Value("Guy")
        );

        assert_eq!(
            parser.attribute_value("authorinitials_4"),
            InterpretedValue::Value("NEG")
        );

        assert_eq!(parser.attribute_value("email_4"), InterpretedValue::Unset);

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Document Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },),
                    title: Some("Document Title",),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[
                            Author {
                                name: "First Second Last",
                                firstname: "First",
                                middlename: Some("Second",),
                                lastname: Some("Last",),
                                email: Some("first@example.com",),
                            },
                            Author {
                                name: "Only First",
                                firstname: "Only",
                                middlename: None,
                                lastname: Some("First",),
                                email: None,
                            },
                            Author {
                                name: "A B C",
                                firstname: "A",
                                middlename: Some("B",),
                                lastname: Some("C",),
                                email: Some("abc@example.com",),
                            },
                            Author {
                                name: "No Email Guy",
                                firstname: "No",
                                middlename: Some("Email",),
                                lastname: Some("Guy",),
                                email: None,
                            },
                        ],
                        source: Span {
                            data: "First Second Last <first@example.com>; Only First; A B C <abc@example.com>; No Email Guy",
                            line: 2,
                            col: 1,
                            offset: 17,
                        },
                    },),
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Document Title\nFirst Second Last <first@example.com>; Only First; A B C <abc@example.com>; No Email Guy",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[],
                source: Span {
                    data: "= Document Title\nFirst Second Last <first@example.com>; Only First; A B C <abc@example.com>; No Email Guy",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }
}
