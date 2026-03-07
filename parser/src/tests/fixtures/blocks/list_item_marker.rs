use std::fmt;

use crate::tests::fixtures::{content::Content, span::Span};

#[derive(Eq, PartialEq)]
pub(crate) enum ListItemMarker {
    Hyphen(Span),
    Asterisks(Span),
    #[allow(unused)] // TEMPORARY while building
    Bullet(Span),
    Dots(Span),
    #[allow(unused)] // TEMPORARY while building
    AlphaListCapital(Span),
    AlphaListLower(Span),
    RomanNumeralLower(Span),
    RomanNumeralUpper(Span),
    ArabicNumeral(Span),

    #[allow(unused)] // TEMPORARY while building
    DefinedTerm {
        term: Content,
        marker: Span,
        source: Span,
    },
}

impl fmt::Debug for ListItemMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hyphen(x) => f.debug_tuple("ListItemMarker::Hyphen").field(x).finish(),
            Self::Asterisks(x) => f.debug_tuple("ListItemMarker::Asterisks").field(x).finish(),
            Self::Bullet(x) => f.debug_tuple("ListItemMarker::Bullet").field(x).finish(),
            Self::Dots(x) => f.debug_tuple("ListItemMarker::Dots").field(x).finish(),

            Self::AlphaListCapital(x) => f
                .debug_tuple("ListItemMarker::AlphaListCapital")
                .field(x)
                .finish(),

            Self::AlphaListLower(x) => f
                .debug_tuple("ListItemMarker::AlphaListLower")
                .field(x)
                .finish(),

            Self::RomanNumeralLower(x) => f
                .debug_tuple("ListItemMarker::RomanNumeralLower")
                .field(x)
                .finish(),

            Self::RomanNumeralUpper(x) => f
                .debug_tuple("ListItemMarker::RomanNumeralUpper")
                .field(x)
                .finish(),

            Self::ArabicNumeral(x) => f
                .debug_tuple("ListItemMarker::ArabicNumeral")
                .field(x)
                .finish(),

            Self::DefinedTerm {
                term,
                marker,
                source,
            } => f
                .debug_struct("ListItemMarker::DefinedTerm")
                .field("term", term)
                .field("marker", marker)
                .field("source", source)
                .finish(),
        }
    }
}

impl PartialEq<crate::blocks::ListItemMarker<'_>> for ListItemMarker {
    fn eq(&self, other: &crate::blocks::ListItemMarker) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<ListItemMarker> for crate::blocks::ListItemMarker<'_> {
    fn eq(&self, other: &ListItemMarker) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(
    fixture: &ListItemMarker,
    observed: &crate::blocks::ListItemMarker<'_>,
) -> bool {
    match fixture {
        ListItemMarker::Hyphen(fixture_span) => match observed {
            crate::blocks::ListItemMarker::Hyphen(observed_span) => fixture_span == observed_span,
            _ => false,
        },

        ListItemMarker::Asterisks(fixture_span) => match observed {
            crate::blocks::ListItemMarker::Asterisks(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::Bullet(fixture_span) => match observed {
            crate::blocks::ListItemMarker::Bullet(observed_span) => fixture_span == observed_span,
            _ => false,
        },

        ListItemMarker::Dots(fixture_span) => match observed {
            crate::blocks::ListItemMarker::Dots(observed_span) => fixture_span == observed_span,
            _ => false,
        },

        ListItemMarker::AlphaListCapital(fixture_span) => match observed {
            crate::blocks::ListItemMarker::AlphaListCapital(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::AlphaListLower(fixture_span) => match observed {
            crate::blocks::ListItemMarker::AlphaListLower(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::RomanNumeralLower(fixture_span) => match observed {
            crate::blocks::ListItemMarker::RomanNumeralLower(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::RomanNumeralUpper(fixture_span) => match observed {
            crate::blocks::ListItemMarker::RomanNumeralUpper(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::ArabicNumeral(fixture_span) => match observed {
            crate::blocks::ListItemMarker::ArabicNumeral(observed_span) => {
                fixture_span == observed_span
            }
            _ => false,
        },

        ListItemMarker::DefinedTerm {
            term: fixture_term,
            marker: fixture_marker,
            source: fixture_source,
        } => match observed {
            crate::blocks::ListItemMarker::DefinedTerm {
                term: observed_term,
                marker: observed_marker,
                source: observed_source,
            } => {
                fixture_term == observed_term
                    && fixture_marker == observed_marker
                    && fixture_source == observed_source
            }
            _ => false,
        },
    }
}
