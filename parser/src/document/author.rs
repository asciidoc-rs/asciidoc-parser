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
/// # Rendered versus raw values
///
/// The [`name`], [`firstname`], [`middlename`], [`lastname`], and [`email`]
/// accessors return the value that populates the corresponding document
/// attribute (`author`, `firstname`, …). As in Asciidoctor, that value has the
/// header substitution group applied — so a literal `<`, `>`, or `&` written on
/// the author line is escaped (`&lt;`, `&gt;`, `&amp;`), matching what
/// `{author}` and the rendered byline emit and what Asciidoctor's own
/// `doc.author` / `authors[0].name` return.
///
/// Each accessor has a `raw_` counterpart ([`raw_name`], [`raw_firstname`],
/// [`raw_middlename`], [`raw_lastname`], [`raw_email`]) that returns the same
/// value *before* that escaping — attribute references still resolved, but the
/// literal special characters left as they were written. This mirrors
/// Asciidoctor's internal, pre-substitution `metadata` hash and lets a consumer
/// that escapes for HTML itself do so exactly once, rather than double-escaping
/// an already-escaped value.
///
/// [author line]: https://docs.asciidoctor.org/asciidoc/latest/document/author-line/
/// [`name`]: Self::name
/// [`firstname`]: Self::firstname
/// [`middlename`]: Self::middlename
/// [`lastname`]: Self::lastname
/// [`email`]: Self::email
/// [`raw_name`]: Self::raw_name
/// [`raw_firstname`]: Self::raw_firstname
/// [`raw_middlename`]: Self::raw_middlename
/// [`raw_lastname`]: Self::raw_lastname
/// [`raw_email`]: Self::raw_email
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Author {
    name: String,
    firstname: String,
    middlename: Option<String>,
    lastname: Option<String>,
    email: Option<String>,

    // The `raw_*` fields hold the same values as their rendered counterparts
    // above, but before the header substitution group escapes any literal `<`,
    // `>`, or `&` written on the author line. Attribute references are still
    // resolved. See the type-level "Rendered versus raw values" documentation.
    raw_name: String,
    raw_firstname: String,
    raw_middlename: Option<String>,
    raw_lastname: Option<String>,
    raw_email: Option<String>,
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
        // substitution to individual components afterwards. Special case: If
        // the entire input is a single attribute reference, treat the
        // expanded result as a single name.
        let is_single_attribute = source.trim().starts_with('{')
            && source.trim().ends_with('}')
            && source.matches('{').count() == 1;

        if is_single_attribute {
            // Entire input is a single attribute reference: Expand and treat as
            // single name.
            let expanded_source = apply_author_subs(source, parser);

            // The raw value expands the reference without applying special
            // characters. A single reference carries no literal special
            // characters of its own, so the raw and rendered expansions differ
            // only if the referenced attribute's own value was already escaped.
            let raw_expanded = resolve_attribute_references(source, parser);

            if names_only {
                // An attribute-entry value is partitioned *after* its
                // references are expanded, so a reference that
                // resolves to a multi-part name (or one with a
                // trailing email) yields the same metadata as the
                // equivalent literal value.
                Some(partition_names_only(&expanded_source, &raw_expanded))
            } else {
                Some(single_name_author(
                    replace_underscores_with_spaces(expanded_source),
                    replace_underscores_with_spaces(raw_expanded),
                ))
            }
        } else if let Some(captures) = AUTHOR.captures(source) {
            // Raw input matches author pattern: Extract each component and
            // apply substitutions to it. The rendered components
            // escape literal special characters
            // (`apply_author_subs`); the raw components resolve
            // attribute references only, leaving those characters as written.
            let (name, firstname, middlename, lastname, email) =
                matched_parts(&captures, |s| apply_author_subs(s, parser));

            let (raw_name, raw_firstname, raw_middlename, raw_lastname, raw_email) =
                matched_parts(&captures, |s| resolve_attribute_references(s, parser));

            Some(Self {
                name,
                firstname,
                middlename,
                lastname,
                email,
                raw_name,
                raw_firstname,
                raw_middlename,
                raw_lastname,
                raw_email,
            })
        } else if source.contains('{') {
            // Input contains attributes that prevent regex match: Expand first,
            // then try parsing.
            let expanded_source = apply_author_subs(source, parser);
            let raw_expanded = resolve_attribute_references(source, parser);

            if let Some(captures) = AUTHOR.captures(&expanded_source) {
                // After expansion, it matches the pattern: Parse normally. The
                // components are already substituted, so the transform is the
                // identity here.
                let (name, firstname, middlename, lastname, email) =
                    matched_parts(&captures, str::to_string);

                // Derive the raw components from the raw expansion, but only
                // when it matches the same pattern *and* agrees
                // on whether a trailing `<email>` is present,
                // so the two representations keep the same
                // structure and differ only in escaping. They can disagree only
                // if a literal author-line bracket was escaped in the rendered
                // value; in that case fall back to the rendered components.
                let (raw_name, raw_firstname, raw_middlename, raw_lastname, raw_email) =
                    match AUTHOR.captures(&raw_expanded) {
                        Some(raw_captures) if raw_captures.get(4).is_some() == email.is_some() => {
                            matched_parts(&raw_captures, str::to_string)
                        }

                        _ => (
                            name.clone(),
                            firstname.clone(),
                            middlename.clone(),
                            lastname.clone(),
                            email.clone(),
                        ),
                    };

                Some(Self {
                    name,
                    firstname,
                    middlename,
                    lastname,
                    email,
                    raw_name,
                    raw_firstname,
                    raw_middlename,
                    raw_lastname,
                    raw_email,
                })
            } else if names_only {
                // An attribute-entry value that still fails the pattern after
                // expansion is partitioned by the names-only rules, so a
                // reference resolving to a four-plus-part name behaves like its
                // literal equivalent. The expanded value is used before any
                // HTML encoding so a trailing `<email>` can
                // still be split off.
                Some(partition_names_only(&expanded_source, &raw_expanded))
            } else {
                // Even after expansion the value does not match the author
                // pattern, so it becomes a single name. The rendered value has
                // the header substitution group applied
                // (escaping any literal `<`, `>`, or `&`),
                // mirroring Asciidoctor's `apply_header_subs`; the
                // raw value expands the references without that escaping.
                Some(single_name_author(
                    replace_underscores_with_spaces(expanded_source),
                    replace_underscores_with_spaces(raw_expanded),
                ))
            }
        } else if names_only {
            // Input comes from an attribute entry (e.g. `:author:`) and does
            // not match the author pattern — typically a name with
            // four or more parts or one containing punctuation such
            // as a comma. Asciidoctor still partitions it by
            // splitting on whitespace into at most three parts,
            // assigning any trailing parts to `lastname`. This path applies no
            // special-characters substitution, so the raw and rendered values
            // are identical.
            Some(partition_names_only(source, source))
        } else {
            // Input doesn't contain attributes and doesn't match the author
            // pattern. Asciidoctor stores the whole line as the author,
            // condensing interior whitespace. Underscores are left literal
            // here: Asciidoctor only converts underscore-joined
            // names while partitioning a *matching* line, not in
            // this fallback.
            //
            // The rendered value has the header substitution group applied, so
            // any literal `<`, `>`, or `&` is escaped — matching
            // Asciidoctor's `apply_header_subs`. The raw value
            // keeps those characters as written.
            let raw_name = condense_whitespace(source);
            let name = apply_author_special_characters(&raw_name, parser);
            Some(single_name_author(name, raw_name))
        }
    }

    /// Parse the single author described by an `:author:` attribute entry.
    ///
    /// `raw` is the entry's raw (pre-substitution) value and `substituted` is
    /// its substituted value (the stored attribute value). When the entry is a
    /// whole-value `pass:[…]` macro its substituted value is the resolved
    /// content, so that value is partitioned — with any generated markup
    /// stripped — rather than the raw macro syntax (see
    /// [`parse_substituted_names_only`]). Every other value is partitioned from
    /// the raw value as before, so plain names, attribute references, and
    /// inline emails are unaffected.
    ///
    /// [`parse_substituted_names_only`]: Self::parse_substituted_names_only
    pub(crate) fn parse_from_entry(
        raw: &str,
        substituted: Option<&str>,
        parser: &Parser,
    ) -> Option<Self> {
        if crate::document::is_attribute_entry_pass_macro(raw) {
            substituted.and_then(Self::parse_substituted_names_only)
        } else {
            Self::parse(raw, parser, true)
        }
    }

    /// Parse a single author from a value that has *already* been through
    /// attribute-value substitution and now carries generated inline HTML —
    /// typically the rendered output of a `pass:[…]` macro in an `:author:`
    /// entry.
    ///
    /// This mirrors the `<`-branch of Asciidoctor's `process_authors` under
    /// `names_only`: the full rendered value — with name-joiner underscores
    /// turned to spaces — becomes the author's `name`, while the name parts are
    /// partitioned from the value with its HTML tags removed, so the formatting
    /// does not leak into `firstname`/`middlename`/`lastname`. As in
    /// Asciidoctor, no email is split off here — an email supplied through a
    /// companion `:email:` entry is attached later.
    pub(crate) fn parse_substituted_names_only(substituted: &str) -> Option<Self> {
        let substituted = substituted.trim();
        if substituted.is_empty() {
            return None;
        }

        let name = replace_underscores_with_spaces(substituted.to_string());
        let stripped = strip_xml_tags(substituted);

        let mut segments = split_whitespace_max3(&stripped);

        if segments.is_empty() {
            // The value was nothing but markup: keep the rendered value as the
            // single name.
            return Some(single_name_author(name.clone(), name));
        }

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

        // This path partitions an already-substituted value, so it applies no
        // further special-characters substitution: the raw values equal the
        // rendered ones.
        Some(Self {
            raw_name: name.clone(),
            raw_firstname: firstname.clone(),
            raw_middlename: middlename.clone(),
            raw_lastname: lastname.clone(),
            raw_email: None,
            name,
            firstname,
            middlename,
            lastname,
            email: None,
        })
    }

    /// Overrides the author's email address, unless `email` is `None`.
    ///
    /// Used when an author is assembled from `author_N` document attributes,
    /// where the name and the companion `email_N` attribute are parsed
    /// separately.
    pub(crate) fn with_email(mut self, email: Option<String>) -> Self {
        if let Some(email) = email {
            // The email arrives from a companion `email_N` attribute value,
            // which carries no author-line special-characters
            // escaping of its own, so the raw and rendered emails
            // are the same.
            self.raw_email = Some(email.clone());
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

    /// Returns the full name of the author *before* the header substitution
    /// group escapes any literal special characters.
    ///
    /// This is [`name`](Self::name) with attribute references resolved but any
    /// literal `<`, `>`, or `&` written on the author line left unescaped,
    /// mirroring Asciidoctor's internal `metadata` hash. Use it when the value
    /// will be HTML-escaped downstream, so the escaping happens exactly once.
    pub fn raw_name(&self) -> &str {
        &self.raw_name
    }

    /// Returns the first name of the author *before* the header substitution
    /// group escapes any literal special characters.
    ///
    /// See [`raw_name`](Self::raw_name) for how the raw value differs from
    /// [`firstname`](Self::firstname).
    pub fn raw_firstname(&self) -> &str {
        &self.raw_firstname
    }

    /// Returns the middle name of the author *before* the header substitution
    /// group escapes any literal special characters.
    ///
    /// See [`raw_name`](Self::raw_name) for how the raw value differs from
    /// [`middlename`](Self::middlename).
    pub fn raw_middlename(&self) -> Option<&str> {
        self.raw_middlename.as_deref()
    }

    /// Returns the last name of the author *before* the header substitution
    /// group escapes any literal special characters.
    ///
    /// See [`raw_name`](Self::raw_name) for how the raw value differs from
    /// [`lastname`](Self::lastname).
    pub fn raw_lastname(&self) -> Option<&str> {
        self.raw_lastname.as_deref()
    }

    /// Returns the email address of the author *before* the header substitution
    /// group escapes any literal special characters.
    ///
    /// See [`raw_name`](Self::raw_name) for how the raw value differs from
    /// [`email`](Self::email).
    pub fn raw_email(&self) -> Option<&str> {
        self.raw_email.as_deref()
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
/// (handled inline in [`Header::parse`](crate::document::Header)) preserves an
/// explicit override, exactly as Asciidoctor does — its `authorinitials`
/// deletion guard lives only in the `author` branch of `process_authors`, not
/// the `authors`/indexed branches.
pub(crate) fn set_author_metadata(parser: &mut Parser, authors: &[Author]) {
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

/// Remove HTML tags from `source`, mirroring Asciidoctor's `XmlSanitizeRx`
/// (`/<[^>]+>/`). Used to partition an author value whose substitution produced
/// inline markup (see [`Author::parse_substituted_names_only`]).
fn strip_xml_tags(source: &str) -> String {
    XML_TAG.replace_all(source, "").into_owned()
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
/// the name — mirroring the email group of the author pattern and Asciidoctor's
/// XML sanitization of a names-only value. The remaining name is then split on
/// whitespace into at most three segments (Ruby's `String#split(nil, 3)`, which
/// also drops leading whitespace). The trailing segment retains its interior
/// text — so a four-plus-part name keeps its later parts in `lastname` — but
/// has repeating spaces condensed to a single space. Each segment then has
/// underscore joiners replaced with spaces.
///
/// `escaped_source` may already have attribute references substituted (the
/// expanded value of a reference such as `{full-name}`); in that case any email
/// it carries is likewise already substituted.
///
/// `raw_source` is the same value expanded *without* the special-characters
/// substitution and is partitioned independently to populate the author's
/// `raw_*` fields (see the [`Author`] documentation). For an
/// attribute entry that carries no literal special characters the two arguments
/// are identical.
fn partition_names_only(escaped_source: &str, raw_source: &str) -> Author {
    // Partition the rendered value first, then partition the raw value using
    // the *rendered* value's trailing-email decision so the two
    // representations keep the same structure and differ only in escaping.
    // A literal author-line bracket is escaped in the rendered value and so
    // is not recognized as an email delimiter; the raw value, whose bracket
    // is still literal, must follow that same decision rather than
    // splitting an email off on its own.
    let escaped = partition_parts(escaped_source, EmailSplit::Detect);

    let raw = partition_parts(
        raw_source,
        if escaped.email.is_some() {
            EmailSplit::Detect
        } else {
            EmailSplit::Suppress
        },
    );

    Author {
        name: escaped.name,
        firstname: escaped.firstname,
        middlename: escaped.middlename,
        lastname: escaped.lastname,
        email: escaped.email,
        raw_name: raw.name,
        raw_firstname: raw.firstname,
        raw_middlename: raw.middlename,
        raw_lastname: raw.lastname,
        raw_email: raw.email,
    }
}

/// The five partitioned strings produced from a single names-only author value.
struct NameParts {
    name: String,
    firstname: String,
    middlename: Option<String>,
    lastname: Option<String>,
    email: Option<String>,
}

/// Whether [`partition_parts`] may split a trailing `<email>` off its input.
enum EmailSplit {
    /// Split a trailing `<email>` off when one is present (the rendered value's
    /// own decision).
    Detect,

    /// Never split an email off; the whole value is treated as the name. Used
    /// for the raw value when the rendered value kept its bracket escaped, so
    /// the two stay structurally aligned.
    Suppress,
}

/// Partition a single names-only author value into its parts, following the
/// rules described on [`partition_names_only`].
fn partition_parts(source: &str, email_split: EmailSplit) -> NameParts {
    let source = source.trim();

    let (name_source, email) = match email_split {
        EmailSplit::Detect => match NAMES_ONLY_EMAIL.captures(source) {
            Some(captures) => (
                captures.get(1).map_or(source, |m| m.as_str()),
                Some(captures[2].to_string()),
            ),
            None => (source, None),
        },

        EmailSplit::Suppress => (source, None),
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

    NameParts {
        name,
        firstname,
        middlename,
        lastname,
        email,
    }
}

/// Extract the author components from a successful [`AUTHOR`] match, applying
/// `transform` to each captured fragment.
///
/// The rendered components pass `apply_author_subs` (special characters, then
/// attribute references); the raw components pass
/// [`resolve_attribute_references`] (attribute references only), so the two
/// share the match's structure but differ in whether literal special characters
/// are escaped. Returns `(name, firstname, middlename, lastname, email)`,
/// promoting a lone middle name to the last name and reconstructing `name` from
/// the parts so interior whitespace is condensed to a single space (matching
/// Asciidoctor).
fn matched_parts<F>(
    captures: &regex::Captures,
    transform: F,
) -> (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
)
where
    F: Fn(&str) -> String,
{
    let firstname = replace_underscores_with_spaces(transform(&captures[1]));

    let mut middlename = captures
        .get(2)
        .map(|m| replace_underscores_with_spaces(transform(m.as_str())));

    let mut lastname = captures
        .get(3)
        .map(|m| replace_underscores_with_spaces(transform(m.as_str())));

    let email = captures.get(4).map(|m| transform(m.as_str()));

    if middlename.is_some() && lastname.is_none() {
        lastname = middlename;
        middlename = None;
    }

    let name = join_name_parts(&firstname, middlename.as_deref(), lastname.as_deref());

    (name, firstname, middlename, lastname, email)
}

/// Build a single-name [`Author`] — one whose whole value is the `name` with no
/// separate first/middle/last/email parts — from its rendered and raw forms.
fn single_name_author(name: String, raw_name: String) -> Author {
    Author {
        firstname: name.clone(),
        name,
        middlename: None,
        lastname: None,
        email: None,
        raw_firstname: raw_name.clone(),
        raw_name,
        raw_middlename: None,
        raw_lastname: None,
        raw_email: None,
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

/// Matches a single HTML tag, mirroring Asciidoctor's `XmlSanitizeRx`.
static XML_TAG: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"<[^>]+>").unwrap()
});

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

/// Returns whether `source` matches the author pattern — at most three
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

/// Apply the header substitution group to an author-line fragment: special
/// characters first, then attribute references — matching Asciidoctor's
/// `apply_header_subs` (`HEADER_SUBS = [:specialcharacters, :attributes]`).
///
/// Running special characters *before* attribute references escapes any literal
/// `<`, `>`, or `&` in the fragment while leaving the expanded value of an
/// attribute reference untouched, so an attribute whose value already contains
/// markup (or a character reference) is inserted verbatim rather than escaped a
/// second time — exactly as Asciidoctor's header subs behave. Numeric character
/// references in the literal text are preserved (see
/// [`apply_author_special_characters`]).
fn apply_author_subs(source: &str, parser: &Parser) -> String {
    use crate::{attributes::element_attribute::SplicedValueEscaping, content::apply_attributes};

    let with_special_characters = apply_author_special_characters(source, parser);

    let span = Span::new(&with_special_characters);
    let mut content = Content::from(span);

    apply_attributes(&mut content, parser, SplicedValueEscaping::Verbatim);

    content.rendered_html().to_string()
}

/// Resolve attribute references in `source` *without* applying the special-
/// characters substitution.
///
/// This yields the raw form of an author fragment — attribute references
/// expanded, but any literal `<`, `>`, or `&` left as written — which populates
/// the [`Author`] `raw_*` fields. It is the attribute-references half of
/// [`apply_author_subs`], mirroring the value Asciidoctor keeps in its
/// internal, pre-substitution `metadata` hash.
///
/// The rendered value is always resolved from the same source first (via
/// [`apply_author_subs`]), which records any `attribute-missing` warning, so
/// this second, raw-value pass discards the warnings it would otherwise
/// duplicate.
fn resolve_attribute_references(source: &str, parser: &Parser) -> String {
    use crate::{attributes::element_attribute::SplicedValueEscaping, content::apply_attributes};

    let warnings_before = parser.substitution_warnings_len();

    let span = Span::new(source);
    let mut content = Content::from(span);

    apply_attributes(&mut content, parser, SplicedValueEscaping::Verbatim);

    parser.truncate_substitution_warnings(warnings_before);

    content.rendered_html().to_string()
}

/// Apply the special-characters substitution to `source`, escaping every
/// literal `<`, `>`, and `&` — except the leading `&` of a numeric HTML
/// character reference, which is left intact so a name such as `AsciiDoc&#174;`
/// keeps its ® entity rather than degrading to `AsciiDoc&amp;#174;` (issue
/// #757).
fn apply_author_special_characters(source: &str, parser: &Parser) -> String {
    let mut result = String::with_capacity(source.len());
    let mut last = 0;

    // Escape the text between the character references, emitting each reference
    // verbatim so its `&` is never doubled.
    for m in NUMERIC_CHARACTER_REFERENCE.find_iter(source) {
        result.push_str(&escape_special_characters(&source[last..m.start()], parser));
        result.push_str(m.as_str());
        last = m.end();
    }

    result.push_str(&escape_special_characters(&source[last..], parser));
    result
}

/// Run the special-characters substitution step over `source`, escaping `<`,
/// `>`, and `&`.
fn escape_special_characters(source: &str, parser: &Parser) -> String {
    use crate::content::apply_special_characters;

    if source.is_empty() {
        return String::new();
    }

    let span = Span::new(source);
    let mut content = Content::from(span);

    apply_special_characters(&mut content, &*parser.renderer);

    content.rendered_html().to_string()
}

/// Matches a numeric HTML character reference — decimal (`&#174;`) or
/// hexadecimal (`&#xAE;`). Mirrors the guard the author line uses when deciding
/// whether a semicolon separates two authors (see
/// [`AuthorLine`](crate::document::AuthorLine)); the leading `&` of such a
/// reference is preserved rather than escaped when header subs are applied.
static NUMERIC_CHARACTER_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"&#(?:[0-9]+|[xX][0-9a-fA-F]+);").unwrap()
});

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::Author;

    // The `raw_*` accessors return the author value with attribute references
    // resolved but the header substitution group's special-characters escaping
    // *not* applied — the value Asciidoctor keeps in its internal `metadata`
    // hash. The rendered accessors are unaffected and keep escaping literal
    // `<`, `>`, and `&` to match `doc.author`/`{author}`.
    mod raw_accessors {
        use crate::{Parser, document::Author};

        // Parse `src` and return its single, first author.
        fn only_author(src: &str) -> Author {
            let mut parser = Parser::default();
            let doc = parser.parse(src);
            doc.authors().first().cloned().unwrap()
        }

        #[test]
        fn plain_name_has_no_special_characters_so_raw_equals_rendered() {
            let a = only_author("= Doc\nKismet R. Lee <kismet@asciidoctor.org>\n\nbody\n");

            assert_eq!(a.name(), "Kismet R. Lee");
            assert_eq!(a.raw_name(), "Kismet R. Lee");
            assert_eq!(a.firstname(), "Kismet");
            assert_eq!(a.raw_firstname(), "Kismet");
            assert_eq!(a.middlename(), Some("R."));
            assert_eq!(a.raw_middlename(), Some("R."));
            assert_eq!(a.lastname(), Some("Lee"));
            assert_eq!(a.raw_lastname(), Some("Lee"));
            assert_eq!(a.email(), Some("kismet@asciidoctor.org"));
            assert_eq!(a.raw_email(), Some("kismet@asciidoctor.org"));
        }

        #[test]
        fn implicit_line_with_literal_angle_brackets_keeps_them_raw() {
            // A line that does not match the author pattern (here because of
            // the comma) becomes a single name. The rendered value
            // escapes the literal angle brackets — matching
            // Asciidoctor's `doc.author` — while the raw
            // value keeps them as written.
            let a = only_author(
                "= Doc\nStuart Rackham, founder of AsciiDoc <founder@asciidoc.org>\n\nbody\n",
            );

            assert_eq!(
                a.name(),
                "Stuart Rackham, founder of AsciiDoc &lt;founder@asciidoc.org&gt;"
            );

            assert_eq!(
                a.raw_name(),
                "Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>"
            );

            assert_eq!(
                a.raw_firstname(),
                "Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>"
            );
        }

        #[test]
        fn literal_ampersand_is_escaped_only_in_the_rendered_value() {
            // A downstream converter that HTML-escapes the raw name gets a
            // single level of escaping rather than the
            // double-escaped `&amp;amp;` it would
            // get from re-escaping the already-escaped rendered value.
            let a = only_author("= Doc\nBen & Jerry\n\nbody\n");

            assert_eq!(a.name(), "Ben &amp; Jerry");
            assert_eq!(a.raw_name(), "Ben & Jerry");
            assert_eq!(a.middlename(), Some("&amp;"));
            assert_eq!(a.raw_middlename(), Some("&"));
        }

        #[test]
        fn four_part_name_with_email_keeps_brackets_raw() {
            let a = only_author("= Doc\nFour Names Not Supported <doc@example.com>\n\nbody\n");

            assert_eq!(a.name(), "Four Names Not Supported &lt;doc@example.com&gt;");
            assert_eq!(a.raw_name(), "Four Names Not Supported <doc@example.com>");
        }

        #[test]
        fn attribute_reference_resolves_then_stays_a_single_raw_name() {
            // The literal `<`/`>` around the referenced email keep the expanded
            // value from matching the author pattern, so it is stored as a
            // single name. The rendered value escapes the brackets;
            // the raw value resolves the references but leaves the
            // brackets literal, and — importantly — keeps the same
            // single-name structure as the rendered value rather
            // than re-splitting into name parts.
            let src = concat!(
                ":first-name: Jane\n",
                ":last-name: Smith\n",
                ":author-email: jane@example.com\n",
                "= Doc\n",
                "{first-name} {last-name} <{author-email}>\n\n",
                "body\n",
            );

            let a = only_author(src);

            assert_eq!(a.name(), "Jane Smith &lt;jane@example.com&gt;");
            assert_eq!(a.raw_name(), "Jane Smith <jane@example.com>");
            assert_eq!(a.middlename(), None);
            assert_eq!(a.raw_middlename(), None);
            assert_eq!(a.lastname(), None);
            assert_eq!(a.raw_lastname(), None);
        }

        #[test]
        fn raw_components_fall_back_when_they_disagree_about_a_trailing_email() {
            // The implicit-author-line counterpart of
            // `names_only_email_split_stays_aligned_between_raw_and_rendered`.
            // Escaping the literal brackets makes the rendered expansion
            // (`Jane Smith &lt;Third&gt;`) match the author pattern as three
            // *names*, while the raw expansion (`Jane Smith <Third>`) matches
            // it as two names plus an `<email>`. The two disagree
            // about whether a trailing email is present, so the raw
            // components fall back to the rendered ones rather than
            // storing a differently-shaped split.
            let src = concat!(
                ":first-name: Jane\n",
                "= Doc\n",
                "{first-name} Smith <Third>\n\n",
                "body\n",
            );

            let a = only_author(src);

            assert_eq!(a.name(), "Jane Smith &lt;Third&gt;");
            assert_eq!(a.email(), None);
            assert_eq!(a.lastname(), Some("&lt;Third&gt;"));

            // The fallback: every raw component equals its rendered
            // counterpart, brackets escaped and all, rather than
            // the `Jane Smith` + `<Third>`-as-email split the raw
            // expansion would have yielded on its own.
            assert_eq!(a.raw_name(), "Jane Smith &lt;Third&gt;");
            assert_eq!(a.raw_firstname(), "Jane");
            assert_eq!(a.raw_middlename(), Some("Smith"));
            assert_eq!(a.raw_lastname(), Some("&lt;Third&gt;"));
            assert_eq!(a.raw_email(), None);
        }

        #[test]
        fn unresolved_attribute_reference_warns_only_once() {
            // The raw pass resolves the same references as the rendered pass,
            // so an author line with an unresolved reference must
            // not record the `attribute-missing` warning twice.
            let mut parser = Parser::default();
            let doc =
                parser.parse(":attribute-missing: warn\n= Doc\nJane {undefined} Smith\n\nbody\n");

            assert_eq!(doc.warnings().count(), 1);
        }

        #[test]
        fn names_only_email_split_stays_aligned_between_raw_and_rendered() {
            // A four-part `:author:` entry that wraps an attribute reference in
            // literal brackets fails the author pattern, so it is partitioned.
            // The rendered value escapes the brackets and keeps no
            // email; the raw value must follow that decision rather
            // than splitting the still- literal `<…>` off as an
            // email, so the two stay structurally aligned.
            let a = only_author(":mail: someone@example.com\n:author: A B C D <{mail}>\n\nbody\n");

            assert_eq!(a.name(), "A B C D &lt;someone@example.com&gt;");
            assert_eq!(a.raw_name(), "A B C D <someone@example.com>");
            assert_eq!(a.email(), None);
            assert_eq!(a.raw_email(), None);
            assert_eq!(a.lastname(), Some("C D &lt;someone@example.com&gt;"));
            assert_eq!(a.raw_lastname(), Some("C D <someone@example.com>"));
        }

        #[test]
        fn names_only_email_from_an_attribute_still_splits_in_both() {
            // When the trailing `<email>` survives into the rendered expansion
            // unescaped (here from an intrinsic attribute value, so the bracket
            // is literal in *both* the rendered and raw
            // expansions), the email is split off in both — the
            // alignment fix only suppresses a raw split
            // the rendered value did not also make.
            let mut parser = Parser::default().with_intrinsic_attribute(
                "tail",
                "Jr. <boss@example.com>",
                crate::parser::ModificationContext::Anywhere,
            );

            let doc = parser.parse("= Doc\n:author: A B C {tail}\n\nbody\n");
            let a = doc.authors().first().cloned().unwrap();

            assert_eq!(a.email(), Some("boss@example.com"));
            assert_eq!(a.raw_email(), Some("boss@example.com"));
        }
    }

    // The `<`-branch of Asciidoctor's `process_authors` (`names_only`), reached
    // when an `:author:` attribute value's substitution produced inline HTML.
    // The rendered markup — with name-joiner underscores turned to spaces —
    // becomes `name`, while the name parts are partitioned from the
    // tag-stripped text so the formatting does not leak into them. No email
    // is split off.
    mod parse_substituted_names_only {
        use super::Author;

        #[test]
        fn empty_input_is_none() {
            assert!(Author::parse_substituted_names_only("").is_none());
            assert!(Author::parse_substituted_names_only("   ").is_none());
        }

        #[test]
        fn markup_with_no_text_keeps_rendered_value_as_single_name() {
            // Stripping the tags leaves nothing to partition, so the rendered
            // markup stands as the whole name and `firstname`.
            let author = Author::parse_substituted_names_only("<a href=\"x\"></a>").unwrap();
            assert_eq!(author.name(), "<a href=\"x\"></a>");
            assert_eq!(author.firstname(), "<a href=\"x\"></a>");
            assert_eq!(author.middlename(), None);
            assert_eq!(author.lastname(), None);
            assert_eq!(author.email(), None);
        }

        #[test]
        fn single_name_part() {
            let author = Author::parse_substituted_names_only("<strong>Solo</strong>").unwrap();
            assert_eq!(author.name(), "<strong>Solo</strong>");
            assert_eq!(author.firstname(), "Solo");
            assert_eq!(author.middlename(), None);
            assert_eq!(author.lastname(), None);
        }

        #[test]
        fn first_and_last_name() {
            let author = Author::parse_substituted_names_only("<em>Kismet</em> Chameleon").unwrap();
            assert_eq!(author.firstname(), "Kismet");
            assert_eq!(author.middlename(), None);
            assert_eq!(author.lastname(), Some("Chameleon"));
        }

        #[test]
        fn first_middle_and_last_name() {
            let author =
                Author::parse_substituted_names_only("<em>Kismet</em> R. Chameleon").unwrap();
            assert_eq!(author.firstname(), "Kismet");
            assert_eq!(author.middlename(), Some("R."));
            assert_eq!(author.lastname(), Some("Chameleon"));
        }

        #[test]
        fn underscores_join_name_parts_and_the_rendered_name() {
            // Underscores act as name joiners: within a part they become a
            // space, and the full `name` has them replaced too.
            let author = Author::parse_substituted_names_only("<b>Ze_Project</b> team").unwrap();
            assert_eq!(author.name(), "<b>Ze Project</b> team");
            assert_eq!(author.firstname(), "Ze Project");
            assert_eq!(author.lastname(), Some("team"));
            assert_eq!(author.email(), None);
        }
    }

    // The entry dispatcher routes a *whole-value* `pass:[…]` macro to its
    // substituted value (the resolved content) and every other value to the
    // raw-value partitioning, so the literal macro syntax never leaks into the
    // name parts.
    mod parse_from_entry {
        use super::Author;
        use crate::Parser;

        #[test]
        fn pass_macro_with_markup_is_partitioned_from_the_substituted_value() {
            // The raw is the macro syntax; the substituted value is its
            // rendered output, which is what gets partitioned (tags
            // stripped).
            let parser = Parser::default();
            let author = Author::parse_from_entry(
                "pass:n[https://example.org/x[Ze *team*]]",
                Some("<a href=\"https://example.org/x\">Ze <strong>team</strong></a>"),
                &parser,
            )
            .unwrap();

            assert_eq!(author.firstname(), "Ze");
            assert_eq!(author.lastname(), Some("team"));
        }

        #[test]
        fn pass_macro_resolving_to_plain_text_uses_the_substituted_value() {
            // A pass macro whose result has no markup must still be partitioned
            // from the substituted value, never the raw `pass:…[…]` syntax.
            let parser = Parser::default();
            let author =
                Author::parse_from_entry("pass:n[Doc Writer]", Some("Doc Writer"), &parser)
                    .unwrap();

            assert_eq!(author.name(), "Doc Writer");
            assert_eq!(author.firstname(), "Doc");
            assert_eq!(author.lastname(), Some("Writer"));
        }

        #[test]
        fn non_pass_value_uses_raw_partitioning() {
            // A value that is not a whole-value pass macro is partitioned from
            // the raw value as before (the substituted value is not consulted).
            let parser = Parser::default();
            let author =
                Author::parse_from_entry("Doc Writer", Some("Doc Writer"), &parser).unwrap();

            assert_eq!(author.firstname(), "Doc");
            assert_eq!(author.lastname(), Some("Writer"));
        }

        #[test]
        fn empty_pass_macro_yields_no_author() {
            // `pass:[]` resolves to an empty value, which describes no author.
            let parser = Parser::default();
            assert!(Author::parse_from_entry("pass:[]", Some(""), &parser).is_none());
        }
    }
}
