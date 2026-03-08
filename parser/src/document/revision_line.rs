use std::sync::LazyLock;

use regex::Regex;

use crate::{
    HasSpan, Parser, Span,
    content::{Content, SubstitutionGroup},
};

/// The revision line is the line directly after the author line in the document
/// header. When the content on this line is structured correctly, the processor
/// assigns the content to the built-in `revnumber`, `revdate`, and `revremark`
/// attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionLine<'src> {
    revnumber: Option<String>,
    revdate: String,
    revremark: Option<String>,
    source: Span<'src>,
}

impl<'src> RevisionLine<'src> {
    pub(crate) fn parse(source: Span<'src>, parser: &mut Parser) -> Self {
        let (left_of_colon, revremark) = if let Some((loc, remark)) = source.split_once(':') {
            (loc.to_owned(), Some(remark.trim().to_owned()))
        } else {
            (source.data().to_owned(), None)
        };

        let (revnumber, revdate) = if let Some((rev, date)) = left_of_colon.split_once(',') {
            // When there's a comma, we have a revision number followed by a date.
            let rev_trimmed = rev.trim();
            let cleaned_rev = strip_non_numeric_prefix(rev_trimmed);
            (Some(cleaned_rev), date.trim().to_owned())
        } else {
            // No comma: Check if this is a standalone revision number.
            let trimmed = left_of_colon.trim();
            if is_valid_standalone_revision(trimmed) {
                // This is a standalone revision number (like "v1.2.3").
                let cleaned_rev = strip_non_numeric_prefix(trimmed);
                (Some(cleaned_rev), String::new())
            } else {
                // This is just a date or other content, not a revision number.
                (None, trimmed.to_owned())
            }
        };

        if let Some(revnumber) = revnumber.as_deref() {
            parser.set_attribute_by_value_from_header("revnumber", revnumber);
        }

        parser.set_attribute_by_value_from_header("revdate", &revdate);

        if let Some(revremark) = revremark.as_deref() {
            parser.set_attribute_by_value_from_header("revremark", revremark);
        }

        Self {
            revnumber: revnumber.map(|s| apply_header_subs(&s, parser)),
            revdate: apply_header_subs(&revdate, parser),
            revremark: revremark.map(|s| apply_header_subs(&s, parser)),
            source,
        }
    }

    /// Returns the revision number, if present.
    ///
    /// The document’s revision number or version is assigned to the built-in
    /// `revnumber` attribute. When assigned using the revision line, the
    /// version must contain at least one number, and, if it isn’t followed by a
    /// date or remark, it must begin with the letter `v` (e.g., `v7.0.6`). Any
    /// letters or symbols preceding the number, including `v`, are dropped when
    /// the document is rendered. If `revnumber` is set with an attribute entry,
    /// it doesn’t have to contain a number and the entire value is displayed in
    /// the rendered document.
    pub fn revnumber(&self) -> Option<&str> {
        self.revnumber.as_deref()
    }

    /// Returns the revision date.
    ///
    /// The date the revision was completed is assigned to the built-in
    /// `revdate` attribute. If the date is assigned using the revision line, it
    /// must be separated from the version by a comma (e.g., `78.1,
    /// 2020-10-10`). The date can contain letters, numbers, symbols, and
    /// attribute references.
    pub fn revdate(&self) -> &str {
        &self.revdate
    }

    /// Returns the revision remark, if present.
    ///
    /// Remarks about the revision of the document are assigned to the built-in
    /// `revremark` attribute. The remark must be separated by a colon (`:`)
    /// from the version or revision date when assigned using the revision line.
    pub fn revremark(&self) -> Option<&str> {
        self.revremark.as_deref()
    }
}

impl<'src> HasSpan<'src> for RevisionLine<'src> {
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

fn is_valid_standalone_revision(s: &str) -> bool {
    STANDALONE_REVISION.is_match(s)
}

fn strip_non_numeric_prefix(s: &str) -> String {
    NON_NUMERIC_PREFIX
        .captures(s)
        .and_then(|captures| captures.get(1))
        .map_or_else(|| s.to_owned(), |m| m.as_str().to_owned())
}

static STANDALONE_REVISION: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^v\d").unwrap()
});

static NON_NUMERIC_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^[^0-9]*(.*)$").unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{Parser, Span};

    #[test]
    fn v_prefix_standalone() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("v1.2.3"), &mut parser);

        assert_eq!(result.revnumber(), Some("1.2.3"));
        assert_eq!(result.revdate(), "");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn standalone_number_without_v_prefix() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("1.2.3"), &mut parser);

        // According to Asciidoctor behavior, standalone numbers without "v" are not
        // revision numbers
        assert_eq!(result.revnumber(), None);
        assert_eq!(result.revdate(), "1.2.3");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn other_prefix_standalone() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("LPR1.2.3"), &mut parser);

        // Other prefixes don't have special standalone treatment
        assert_eq!(result.revnumber(), None);
        assert_eq!(result.revdate(), "LPR1.2.3");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn v_prefix_with_comma_and_date() {
        let mut parser = Parser::default();
        let result =
            crate::document::RevisionLine::parse(Span::new("v1.2.3, 2023-01-15"), &mut parser);

        assert_eq!(result.revnumber(), Some("1.2.3"));
        assert_eq!(result.revdate(), "2023-01-15");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn other_prefix_with_comma_and_date() {
        let mut parser = Parser::default();
        let result =
            crate::document::RevisionLine::parse(Span::new("LPR1.2.3, 2023-01-15"), &mut parser);

        // With comma, other prefixes should be stripped from revision number
        assert_eq!(result.revnumber(), Some("1.2.3"));
        assert_eq!(result.revdate(), "2023-01-15");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn revision_with_colon_and_remark() {
        let mut parser = Parser::default();
        let result =
            crate::document::RevisionLine::parse(Span::new("v1.2.3: A great release"), &mut parser);

        assert_eq!(result.revnumber(), Some("1.2.3"));
        assert_eq!(result.revdate(), "");
        assert_eq!(result.revremark(), Some("A great release"));
    }

    #[test]
    fn full_revision_line() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(
            Span::new("v2.1.0, 2023-12-25: Christmas release"),
            &mut parser,
        );

        assert_eq!(result.revnumber(), Some("2.1.0"));
        assert_eq!(result.revdate(), "2023-12-25");
        assert_eq!(result.revremark(), Some("Christmas release"));
    }

    #[test]
    fn only_date() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("2023-01-15"), &mut parser);

        // Just a date, no revision number
        assert_eq!(result.revnumber(), None);
        assert_eq!(result.revdate(), "2023-01-15");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn date_with_remark() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(
            Span::new("2023-01-15: New year update"),
            &mut parser,
        );

        assert_eq!(result.revnumber(), None);
        assert_eq!(result.revdate(), "2023-01-15");
        assert_eq!(result.revremark(), Some("New year update"));
    }

    #[test]
    fn whitespace_handling() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(
            Span::new("  v1.0.0  ,   Jan 1, 2023   :   Initial release  "),
            &mut parser,
        );

        assert_eq!(result.revnumber(), Some("1.0.0"));
        assert_eq!(result.revdate(), "Jan 1, 2023");
        assert_eq!(result.revremark(), Some("Initial release"));
    }

    #[test]
    fn v_only_no_digits() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("v"), &mut parser);

        // "v" without digits should not be treated as a standalone revision
        assert_eq!(result.revnumber(), None);
        assert_eq!(result.revdate(), "v");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn complex_version_with_v() {
        let mut parser = Parser::default();
        let result = crate::document::RevisionLine::parse(Span::new("v1.2.3-beta.1"), &mut parser);

        assert_eq!(result.revnumber(), Some("1.2.3-beta.1"));
        assert_eq!(result.revdate(), "");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn numeric_prefix_stripped() {
        let mut parser = Parser::default();
        let result =
            crate::document::RevisionLine::parse(Span::new("abc123def, 2023-01-01"), &mut parser);

        // Non-numeric prefix should be stripped, leaving "123def"
        assert_eq!(result.revnumber(), Some("123def"));
        assert_eq!(result.revdate(), "2023-01-01");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn no_numeric_content() {
        let mut parser = Parser::default();
        let result =
            crate::document::RevisionLine::parse(Span::new("nodigits, 2023-01-01"), &mut parser);

        // When there are no digits, the prefix stripping should leave empty string
        assert_eq!(result.revnumber(), Some(""));
        assert_eq!(result.revdate(), "2023-01-01");
        assert_eq!(result.revremark(), None);
    }

    #[test]
    fn sets_document_attributes_with_all_components() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(
            Span::new("v2.1.0, 2023-12-25: Christmas release"),
            &mut parser,
        );

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("2.1.0")
        );

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("2023-12-25")
        );

        assert_eq!(
            parser.attribute_value("revremark").as_maybe_str(),
            Some("Christmas release")
        );
    }

    #[test]
    fn sets_document_attributes_revision_number_only() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(Span::new("v1.2.3"), &mut parser);

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("1.2.3")
        );

        assert_eq!(parser.attribute_value("revdate").as_maybe_str(), Some(""));
        assert_eq!(parser.attribute_value("revremark").as_maybe_str(), None);
    }

    #[test]
    fn sets_document_attributes_date_only() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(Span::new("2023-01-15"), &mut parser);

        assert_eq!(parser.attribute_value("revnumber").as_maybe_str(), None);

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("2023-01-15")
        );

        assert_eq!(parser.attribute_value("revremark").as_maybe_str(), None);
    }

    #[test]
    fn sets_document_attributes_date_with_remark() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(
            Span::new("2023-01-15: New year update"),
            &mut parser,
        );

        assert_eq!(parser.attribute_value("revnumber").as_maybe_str(), None);

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("2023-01-15")
        );

        assert_eq!(
            parser.attribute_value("revremark").as_maybe_str(),
            Some("New year update")
        );
    }

    #[test]
    fn sets_document_attributes_revision_with_date() {
        let mut parser = Parser::default();
        let _result =
            crate::document::RevisionLine::parse(Span::new("v1.2.3, 2023-01-15"), &mut parser);

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("1.2.3")
        );

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("2023-01-15")
        );

        assert_eq!(parser.attribute_value("revremark").as_maybe_str(), None);
    }

    #[test]
    fn sets_document_attributes_revision_with_remark_only() {
        let mut parser = Parser::default();
        let _result =
            crate::document::RevisionLine::parse(Span::new("v1.2.3: A great release"), &mut parser);

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("1.2.3")
        );

        assert_eq!(parser.attribute_value("revdate").as_maybe_str(), Some(""));

        assert_eq!(
            parser.attribute_value("revremark").as_maybe_str(),
            Some("A great release")
        );
    }

    #[test]
    fn sets_document_attributes_with_whitespace_handling() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(
            Span::new("  v1.0.0  ,   Jan 1, 2023   :   Initial release  "),
            &mut parser,
        );

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("1.0.0")
        );

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("Jan 1, 2023")
        );

        assert_eq!(
            parser.attribute_value("revremark").as_maybe_str(),
            Some("Initial release")
        );
    }

    #[test]
    fn sets_document_attributes_with_prefix_stripping() {
        let mut parser = Parser::default();
        let _result =
            crate::document::RevisionLine::parse(Span::new("abc123def, 2023-01-01"), &mut parser);

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("123def")
        );

        assert_eq!(
            parser.attribute_value("revdate").as_maybe_str(),
            Some("2023-01-01")
        );

        assert_eq!(parser.attribute_value("revremark").as_maybe_str(), None);
    }

    #[test]
    fn sets_document_attributes_complex_version() {
        let mut parser = Parser::default();
        let _result = crate::document::RevisionLine::parse(Span::new("v1.2.3-beta.1"), &mut parser);

        assert_eq!(
            parser.attribute_value("revnumber").as_maybe_str(),
            Some("1.2.3-beta.1")
        );

        assert_eq!(parser.attribute_value("revdate").as_maybe_str(), Some(""));
        assert_eq!(parser.attribute_value("revremark").as_maybe_str(), None);
    }
}
