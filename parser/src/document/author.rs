use std::sync::LazyLock;

use regex::Regex;

use crate::{Parser, Span, content::Content};

/// Represents a single author as (typically) described on the [author line].
///
/// The attributes `firstname`, `middlename`, `lastname`, and `authorinitials`
/// are automatically derived from the full value of the author string. When
/// assigned implicitly via the author line, the value includes all of the
/// characters and words prior to the semicolon (`;`), angle bracket (`<`), or
/// the end of the line. Note that when using the implicit author line, the full
/// name can have a maximum of three space-separated names. If it has more, then
/// the full name is assigned to the `firstname` attribute. You can adjoin names
/// using an underscore (`_`) character.
///
/// [author line]: https://docs.asciidoctor.org/asciidoc/latest/document/author-line/
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Author {
    name: String,
    firstname: String,
    middlename: Option<String>,
    lastname: Option<String>,
    email: Option<String>,
}

impl Author {
    /// Parse a single author from `source`.
    ///
    /// `names_only` distinguishes the two contexts in which Asciidoctor parses
    /// an author. The implicit author line (`names_only == false`) recognizes
    /// at most three space-separated names via the [`AUTHOR`] pattern and,
    /// failing that, stores the whole line as the author. An author
    /// supplied through an attribute entry such as `:author:` (`names_only
    /// == true`) is instead partitioned by splitting on whitespace into at
    /// most three parts, so a name with four or more parts still assigns
    /// its trailing parts to `lastname` (see issue #758).
    pub(crate) fn parse(source: &str, parser: &Parser, names_only: bool) -> Option<Self> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }

        // Parse the raw input first to extract components, then apply attribute
        // substitution to individual components afterwards. Special case: If the entire
        // input is a single attribute reference, treat the expanded result as a single
        // name.
        let is_single_attribute = source.trim().starts_with('{')
            && source.trim().ends_with('}')
            && source.matches('{').count() == 1;

        if is_single_attribute {
            // Entire input is a single attribute reference: Expand and treat as single
            // name.
            let expanded_source = apply_author_subs(source, parser);

            if names_only {
                // An attribute-entry value is partitioned *after* its references
                // are expanded, so a reference that resolves to a multi-part name
                // (or one with a trailing email) yields the same metadata as the
                // equivalent literal value.
                Some(partition_names_only(&expanded_source))
            } else {
                let name_with_spaces = replace_underscores_with_spaces(expanded_source);
                Some(Self {
                    name: name_with_spaces.clone(),
                    firstname: name_with_spaces,
                    middlename: None,
                    lastname: None,
                    email: None,
                })
            }
        } else if let Some(captures) = AUTHOR.captures(source) {
            // Raw input matches author pattern: Extract components then apply
            // substitutions.

            // Extract raw components first.
            let firstname =
                replace_underscores_with_spaces(apply_author_subs(&captures[1], parser));
            let mut middlename = captures
                .get(2)
                .map(|m| replace_underscores_with_spaces(apply_author_subs(m.as_str(), parser)));
            let mut lastname = captures
                .get(3)
                .map(|m| replace_underscores_with_spaces(apply_author_subs(m.as_str(), parser)));
            let email = captures
                .get(4)
                .map(|m| apply_author_subs(m.as_str(), parser));

            if middlename.is_some() && lastname.is_none() {
                lastname = middlename;
                middlename = None;
            }

            // Reconstruct the full name from its parsed parts so that any interior
            // whitespace that appeared between the names in the source is condensed
            // to a single space (matching Asciidoctor).
            let name = join_name_parts(&firstname, middlename.as_deref(), lastname.as_deref());

            Some(Self {
                name,
                firstname,
                middlename,
                lastname,
                email,
            })
        } else if source.contains('{') {
            // Input contains attributes that prevent regex match: Expand first, then try
            // parsing.
            let expanded_source = apply_author_subs(source, parser);

            if let Some(captures) = AUTHOR.captures(&expanded_source) {
                // After expansion, it matches the pattern: Parse normally.
                let firstname = replace_underscores_with_spaces(captures[1].to_string());
                let mut middlename = captures
                    .get(2)
                    .map(|m| replace_underscores_with_spaces(m.as_str().to_string()));
                let mut lastname = captures
                    .get(3)
                    .map(|m| replace_underscores_with_spaces(m.as_str().to_string()));
                let email = captures.get(4).map(|m| m.as_str().to_string());

                if middlename.is_some() && lastname.is_none() {
                    lastname = middlename;
                    middlename = None;
                }

                // Reconstruct the full name from its parsed parts so interior
                // whitespace between the names is condensed (matching Asciidoctor).
                let name = join_name_parts(&firstname, middlename.as_deref(), lastname.as_deref());

                Some(Self {
                    name,
                    firstname,
                    middlename,
                    lastname,
                    email,
                })
            } else if names_only {
                // An attribute-entry value that still fails the pattern after
                // expansion is partitioned by the names-only rules, so a
                // reference resolving to a four-plus-part name behaves like its
                // literal equivalent. The expanded value is used before any HTML
                // encoding so a trailing `<email>` can still be split off.
                Some(partition_names_only(&expanded_source))
            } else {
                // Even after expansion, doesn't match: Treat as single name with HTML encoding.
                let mut expanded_name = expanded_source;

                if expanded_name.contains('<') && expanded_name.contains('>') {
                    let span = crate::Span::new(&expanded_name);
                    let mut content = crate::content::Content::from(span);
                    crate::content::SubstitutionStep::SpecialCharacters.apply(
                        &mut content,
                        parser,
                        None,
                    );
                    expanded_name = content.rendered().to_string();
                }

                let name_with_spaces = replace_underscores_with_spaces(expanded_name);
                Some(Self {
                    name: name_with_spaces.clone(),
                    firstname: name_with_spaces,
                    middlename: None,
                    lastname: None,
                    email: None,
                })
            }
        } else if names_only {
            // Input comes from an attribute entry (e.g. `:author:`) and does not
            // match the author pattern – typically a name with four or more parts
            // or one containing punctuation such as a comma. Asciidoctor still
            // partitions it by splitting on whitespace into at most three parts,
            // assigning any trailing parts to `lastname`.
            Some(partition_names_only(source))
        } else {
            // Input doesn't contain attributes and doesn't match the author pattern.
            // Asciidoctor stores the whole line as the author, condensing interior
            // whitespace and keeping any angle brackets literal. Underscores are left
            // literal here: Asciidoctor only converts underscore-joined names while
            // partitioning a *matching* line, not in this fallback.
            let name = condense_whitespace(source);
            Some(Self {
                name: name.clone(),
                firstname: name,
                middlename: None,
                lastname: None,
                email: None,
            })
        }
    }

    /// Overrides the author's email address, unless `email` is `None`.
    ///
    /// Used when an author is assembled from `author_N` document attributes,
    /// where the name and the companion `email_N` attribute are parsed
    /// separately.
    pub(crate) fn with_email(mut self, email: Option<String>) -> Self {
        if let Some(email) = email {
            self.email = Some(email);
        }

        self
    }

    /// Returns the full name of the author.
    ///
    /// The name includes the entire author declaration except for email.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the first, forename, or given name of the author.
    ///
    /// The first space-separated name in the value of the `author` attribute is
    /// automatically assigned to `firstname`.
    pub fn firstname(&self) -> &str {
        &self.firstname
    }

    /// Returns the middle name or initial of the author.
    ///
    /// If author contains three space-separated names, the second name is
    /// assigned to the `middlename` attribute.
    pub fn middlename(&self) -> Option<&str> {
        self.middlename.as_deref()
    }

    /// Returns the last, surname, or family name of the author.
    ///
    /// If the author name contains two or three space-separated names, the last
    /// of those names is assigned to the `lastname` attribute.
    pub fn lastname(&self) -> Option<&str> {
        self.lastname.as_deref()
    }

    /// Returns the email address or URL associated with the author.
    ///
    /// When assigned via the author line, it’s enclosed in a pair of angle
    /// brackets (`< >`). A URL can be used in place of the email address.
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Returns the initials of the author.
    ///
    /// The first character of the `firstname`, `middlename`, and `lastname`
    /// attribute values are assigned to the `authorinitials` attribute. The
    /// value of the `authorinitials` attribute will consist of three characters
    /// or less depending on how many parts are in the author’s name.
    pub fn initials(&self) -> String {
        format!(
            "{first}{middle}{last}",
            first = first_char_or_empty_string(&self.firstname),
            middle = opt_first_char_or_empty_string(self.middlename.as_deref()),
            last = opt_first_char_or_empty_string(self.lastname.as_deref()),
        )
    }
}

fn first_char_or_empty_string(s: &str) -> String {
    s.chars().next().map_or(String::new(), |c| c.to_string())
}

fn opt_first_char_or_empty_string(s: Option<&str>) -> String {
    s.map(first_char_or_empty_string).unwrap_or_default()
}

/// Replace underscores with spaces in a name component.
fn replace_underscores_with_spaces(name: String) -> String {
    name.replace('_', " ")
}

/// Join an author's parsed name parts with a single space.
///
/// Asciidoctor reconstructs the full name from its partitioned parts, which
/// condenses any interior whitespace that appeared between the names in the
/// source down to a single space.
fn join_name_parts(firstname: &str, middlename: Option<&str>, lastname: Option<&str>) -> String {
    let mut name = String::from(firstname);

    if let Some(middlename) = middlename {
        name.push(' ');
        name.push_str(middlename);
    }

    if let Some(lastname) = lastname {
        name.push(' ');
        name.push_str(lastname);
    }

    name
}

/// Partition an author value that does not match the [`AUTHOR`] pattern using
/// Asciidoctor's `names_only` rules (the path taken for an attribute-entry
/// value such as `:author:`).
///
/// A trailing `<email>` (or URL) is first split off so it is not absorbed into
/// the name – mirroring the email group of the author pattern and Asciidoctor's
/// XML sanitization of a names-only value. The remaining name is then split on
/// whitespace into at most three segments (Ruby's `String#split(nil, 3)`, which
/// also drops leading whitespace). The trailing segment retains its interior
/// text – so a four-plus-part name keeps its later parts in `lastname` – but
/// has repeating spaces condensed to a single space. Each segment then has
/// underscore joiners replaced with spaces.
///
/// `source` may already have attribute references substituted (the expanded
/// value of a reference such as `{full-name}`); in that case any email it
/// carries is likewise already substituted.
fn partition_names_only(source: &str) -> Author {
    let source = source.trim();

    let (name_source, email) = match NAMES_ONLY_EMAIL.captures(source) {
        Some(captures) => (
            captures.get(1).map_or(source, |m| m.as_str()),
            Some(captures[2].to_string()),
        ),
        None => (source, None),
    };

    let mut segments = split_whitespace_max3(name_source);

    let firstname = replace_underscores_with_spaces(segments.remove(0));
    let (middlename, lastname) = match segments.len() {
        0 => (None, None),
        1 => (
            None,
            Some(replace_underscores_with_spaces(segments.remove(0))),
        ),
        _ => (
            Some(replace_underscores_with_spaces(segments.remove(0))),
            Some(replace_underscores_with_spaces(segments.remove(0))),
        ),
    };

    let name = join_name_parts(&firstname, middlename.as_deref(), lastname.as_deref());

    Author {
        name,
        firstname,
        middlename,
        lastname,
        email,
    }
}

/// Split `source` on runs of whitespace into at most three segments, mirroring
/// Ruby's `String#split(nil, 3)`. Leading whitespace is dropped and the first
/// two whitespace runs delimit the first two segments; the remainder becomes
/// the third segment, with its repeating spaces condensed to a single space
/// (Ruby's `String#squeeze ' '`).
///
/// Only ASCII whitespace is treated as a delimiter, matching Ruby's split
/// (which does not break on non-breaking or other Unicode spaces), so a name
/// joined by such a space stays a single segment. The returned vector always
/// has at least one element because the caller has already rejected empty
/// input.
fn split_whitespace_max3(source: &str) -> Vec<String> {
    let is_ascii_ws = |c: char| c.is_ascii_whitespace();

    let mut segments: Vec<String> = Vec::with_capacity(3);
    let mut rest = source;

    for _ in 0..2 {
        rest = rest.trim_start_matches(is_ascii_ws);
        match rest.find(is_ascii_ws) {
            Some(index) => {
                segments.push(rest[..index].to_string());
                rest = &rest[index..];
            }
            None => break,
        }
    }

    rest = rest.trim_start_matches(is_ascii_ws);
    if !rest.is_empty() {
        segments.push(condense_whitespace(rest));
    }

    segments
}

/// Condense runs of spaces into a single space, mirroring Ruby's
/// `String#tr_s(' ', ' ')`, which Asciidoctor applies to an author line that
/// does not match the author pattern.
fn condense_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_was_space = false;

    for c in s.chars() {
        if c == ' ' {
            if !prev_was_space {
                result.push(' ');
            }
            prev_was_space = true;
        } else {
            result.push(c);
            prev_was_space = false;
        }
    }

    result
}

static AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
            ^

            # Group 1: First name (required)
            ([a-zA-Z0-9_\p{L}\p{N}&\#;][a-zA-Z0-9_\p{L}\p{N}\-'.&\#;]*)

            # Group 2: Middle name (optional)
            (?:\ +([a-zA-Z0-9_\p{L}\p{N}&\#;][a-zA-Z0-9_\p{L}\p{N}\-'.&\#;]*))?

            # Group 3: Last name (optional)
            (?:\ +([a-zA-Z0-9_\p{L}\p{N}&\#;][a-zA-Z0-9_\p{L}\p{N}\-'.&\#;]*))?

            # Group 4: Email address (optional)
            (?:\ +<([^>]+)>)?

            $
        "#,
    )
    .unwrap()
});

/// Splits a names-only author value into its name portion (group 1) and a
/// trailing `<email>` (group 2). The name must contain at least one
/// non-whitespace character and is followed by whitespace before the bracketed
/// email, matching the email group of [`AUTHOR`] for a value that otherwise
/// fails the full pattern (e.g. a name with four or more parts).
static NAMES_ONLY_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^(.*\S)\s+<([^>]+)>$").unwrap()
});

/// Returns whether `source` matches the author pattern – at most three
/// space-separated names with an optional trailing `<email>`.
///
/// The `:author:` attribute-entry path uses this to tell whether a plain-name
/// value was partitioned by the fallback whitespace split (a name with four or
/// more parts, or one containing punctuation such as a comma) rather than by
/// the pattern. Only in the fallback case is the stored `author` value replaced
/// with the reconstructed, whitespace-condensed name (issue #758).
pub(crate) fn matches_author_pattern(source: &str) -> bool {
    AUTHOR.is_match(source.trim())
}

fn apply_author_subs(source: &str, parser: &Parser) -> String {
    let span = Span::new(source);
    let mut content = Content::from(span);

    use crate::content::SubstitutionStep;

    // Apply attribute references first.
    SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);

    // Apply HTML encoding:
    // - Single attribute reference (like {full-author}): No HTML encoding.
    // - Single attribute in email position (like <{email}>): No HTML encoding.
    // - Multiple attributes or complex patterns: HTML encoding.
    // - Don't HTML encode if the content only has pre-existing HTML entities.
    let is_simple_single_attribute = source.trim().starts_with('{')
        && source.trim().ends_with('}')
        && source.matches('{').count() == 1;

    let has_multiple_attributes = source.matches('{').count() > 1;

    // Check if we should apply HTML encoding.
    let rendered = content.rendered();
    let has_angle_brackets = rendered.contains('<') && rendered.contains('>');
    let has_unencoded_ampersand = rendered.contains('&') && !rendered.contains("&amp;");

    if !is_simple_single_attribute
        && has_multiple_attributes
        && (has_angle_brackets || has_unencoded_ampersand)
    {
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
    }

    content.rendered().to_string()
}
