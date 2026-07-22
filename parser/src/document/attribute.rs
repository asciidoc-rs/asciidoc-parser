use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{ContentModel, IsBlock},
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    strings::CowStr,
};

/// Document attributes are effectively document-scoped variables for the
/// AsciiDoc language. The AsciiDoc language defines a set of built-in
/// attributes, and also allows the author (or extensions) to define additional
/// document attributes, which may replace built-in attributes when permitted.
///
/// An attribute entry is most often declared in the document header. For
/// attributes that allow it (which includes general purpose attributes), the
/// attribute entry can alternately be declared between blocks in the document
/// body (i.e., the portion of the document below the header).
///
/// When an attribute is defined in the document body using an attribute entry,
/// that’s simply referred to as a document attribute. For any attribute defined
/// in the body, the attribute is available from the point it is set until it is
/// unset. Attributes defined in the body are not available via the document
/// metadata.
///
/// An attribute declared between blocks (i.e. in the document body) is
/// represented in this using the same structure (`Attribute`) as a header
/// attribute. Since it lives between blocks, we treat it as though it was a
/// block (and thus implement [`IsBlock`] on this type) even though is not
/// technically a block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute<'src> {
    name: Span<'src>,
    value_source: Option<Span<'src>>,
    value: InterpretedValue,
    source: Span<'src>,
}

impl<'src> Attribute<'src> {
    pub(crate) fn parse(source: Span<'src>, parser: &Parser) -> Option<MatchedItem<'src, Self>> {
        let colon = source.take_prefix(":")?;

        let mut unset = false;
        let line = if colon.after.starts_with('!') {
            unset = true;
            colon.after.slice_from(1..)
        } else {
            colon.after
        };

        let name = line.take_user_attr_name()?;

        // A trailing `!` on the name (immediately before the closing colon) is
        // the postfix unset marker, e.g. `:foo!:`. It is stripped from the
        // stored name. A name may not carry both a leading and a trailing `!`.
        let (name_item, after_name) = if name.item.data().ends_with('!') {
            if unset {
                return None;
            }
            unset = true;
            let len = name.item.data().len();
            (name.item.slice_to(..len - 1), name.after)
        } else {
            (name.item, name.after)
        };

        // `line.after` begins immediately after the closing `:` and runs to the
        // end of the source. The value (and any continuation lines) live here.
        let line = after_name.take_prefix(":")?;

        let (value, value_source, after) = if unset {
            // The value (if any) is ignored, but any continuation lines are
            // still consumed so they don't leak into the block stream.
            let extent = line
                .after
                .take_whitespace()
                .after
                .take_value_with_continuation();
            (InterpretedValue::Unset, None, extent.after)
        } else {
            // The first line's content after the closing colon, with trailing
            // spaces trimmed, decides whether this is a set-only entry.
            let first_line = line.after.take_normalized_line();

            if first_line.item.is_empty() {
                // `:name:` with nothing (but optional trailing whitespace) after
                // the closing colon is a set-only entry.
                (InterpretedValue::Set, None, first_line.after)
            } else {
                // Asciidoctor requires at least one space or tab between the
                // closing colon and the value. A non-blank character immediately
                // after the colon means the line is not a valid attribute entry:
                // the name either contains a colon (`:foo:bar: baz`) or ends with
                // one (`:foo:: bar`), so the whole line falls through to be parsed
                // as an ordinary block. See #728.
                let extent = line
                    .after
                    .take_required_whitespace()?
                    .after
                    .take_value_with_continuation();
                (
                    InterpretedValue::from_raw_value(&extent.item, parser),
                    Some(extent.item),
                    extent.after,
                )
            }
        };

        let source = source.trim_remainder(after);
        Some(MatchedItem {
            item: Self {
                name: name_item,
                value_source,
                value,
                source: source.trim_trailing_whitespace(),
            },
            after,
        })
    }

    /// Return a [`Span`] describing the attribute name.
    pub fn name(&'src self) -> &'src Span<'src> {
        &self.name
    }

    /// Return a [`Span`] containing the attribute's raw value (if present).
    pub fn raw_value(&'src self) -> Option<Span<'src>> {
        self.value_source
    }

    /// Return the attribute's interpolated value.
    pub fn value(&'src self) -> &'src InterpretedValue {
        &self.value
    }
}

impl<'src> HasSpan<'src> for Attribute<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl<'src> IsBlock<'src> for Attribute<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Empty
    }

    fn raw_context(&self) -> CowStr<'src> {
        "attribute".into()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        None
    }

    fn title(&self) -> Option<&str> {
        None
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        None
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        None
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        None
    }
}

/// The interpreted value of an [`Attribute`].
///
/// If the value contains a textual value, this value will
/// have any continuation markers resolved, but will no longer
/// contain a reference to the [`Span`] that contains the value.
#[derive(Clone, Eq, PartialEq)]
pub enum InterpretedValue {
    /// A custom value with all necessary interpolations applied.
    Value(String),

    /// No explicit value. This is typically interpreted as either
    /// boolean `true` or a default value for a built-in attribute.
    Set,

    /// Explicitly unset. This is typically interpreted as boolean `false`.
    Unset,
}

impl std::fmt::Debug for InterpretedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretedValue::Value(value) => f
                .debug_tuple("InterpretedValue::Value")
                .field(value)
                .finish(),

            InterpretedValue::Set => write!(f, "InterpretedValue::Set"),
            InterpretedValue::Unset => write!(f, "InterpretedValue::Unset"),
        }
    }
}

impl InterpretedValue {
    fn from_raw_value(raw_value: &Span<'_>, parser: &Parser) -> Self {
        let mut content = Content::from(*raw_value);

        // Fold any soft-wrap (`\`) or legacy (`+`) line continuation. When there
        // is no continuation marker, the value is a plain single line and is
        // left untouched.
        if let Some(folded) = fold_continuation_value(raw_value.data()) {
            content.rendered = CowStr::Boxed(folded.into_boxed_str());
        }

        SubstitutionGroup::Header.apply(&mut content, parser, None);

        InterpretedValue::Value(content.rendered.into_string())
    }

    pub(crate) fn as_maybe_str(&self) -> Option<&str> {
        match self {
            InterpretedValue::Value(value) => Some(value.as_ref()),
            _ => None,
        }
    }
}

/// ASCII whitespace stripped by Ruby's `String#lstrip` / `#rstrip`, which
/// Asciidoctor applies while fusing a continued attribute value. Unicode
/// whitespace (e.g. a non-breaking space) is significant and is preserved.
const ASCII_WHITESPACE: [char; 6] = [' ', '\t', '\n', '\r', '\x0C', '\x0B'];

/// Fold an attribute entry value that carries a soft-wrap (`\`) or legacy (`+`)
/// line continuation across physical lines, mirroring Asciidoctor's
/// `Parser.process_attribute_entry`.
///
/// The continuation marker is fixed by the first line: a modern soft-wrap
/// marker (a space followed by `\`) or a legacy marker (a space followed by
/// `+`). Continued lines are left-trimmed and, while they carry the same
/// marker, keep the value open. Lines are joined with a newline when the
/// accumulated value ends in a hard line break marker (` +`), otherwise they
/// are folded into a single space.
///
/// The `+` of a preserved hard line break is left in the value verbatim: the
/// `post_replacements` substitution step is not applied to attribute entry
/// values, so the `+` is only interpreted as a line break later, when the value
/// is used in a block whose substitutions include `post_replacements`.
///
/// Returns `None` when the value has no continuation marker (a plain
/// single-line value that needs no folding).
fn fold_continuation_value(data: &str) -> Option<String> {
    const HARD_LINE_BREAK: &str = " +";

    let mut lines = data
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));

    // `str::split` always yields at least one item, so `first` is always present.
    let first = lines.next().unwrap_or_default();

    // The continuation marker is fixed by the first line. Without one, the value
    // is a single line and needs no folding.
    let con: &str = if first.ends_with(" \\") {
        " \\"
    } else if first.ends_with(" +") {
        " +"
    } else {
        return None;
    };

    // Drop the marker from the first line and right-trim (Ruby `rstrip`).
    let mut value = first[..first.len() - con.len()]
        .trim_end_matches(ASCII_WHITESPACE)
        .to_string();

    for line in lines {
        // A blank line (or end of input) terminates the value.
        if line.is_empty() {
            break;
        }

        // Continuation lines are left-trimmed (Ruby `lstrip`).
        let line = line.trim_start_matches(ASCII_WHITESPACE);

        // A line still carrying the marker keeps the value open; strip the
        // marker (and right-trim) before appending.
        let keep_open = line.ends_with(con);
        let piece = if keep_open {
            line[..line.len() - con.len()].trim_end_matches(ASCII_WHITESPACE)
        } else {
            line
        };

        // Join with a newline when the accumulated value ends in a hard line
        // break marker (` +`), otherwise fold into a single space.
        value.push_str(if value.ends_with(HARD_LINE_BREAK) {
            "\n"
        } else {
            " "
        });

        value.push_str(piece);

        if !keep_open {
            break;
        }
    }

    Some(value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{blocks::ContentModel, tests::prelude::*};

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let h1 =
            crate::document::Attribute::parse(crate::Span::new(":foo: bar"), &Parser::default())
                .unwrap();
        let h2 = h1.clone();
        assert_eq!(h1, h2);
    }

    #[test]
    fn simple_value() {
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":foo: bar\nblah"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
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
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar"));

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 2,
                col: 1,
                offset: 10
            }
        );
    }

    #[test]
    fn no_value() {
        let mi =
            crate::document::Attribute::parse(crate::Span::new(":foo:\nblah"), &Parser::default())
                .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":foo:",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Set);

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 2,
                col: 1,
                offset: 6
            }
        );
    }

    #[test]
    fn name_with_hyphens() {
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":name-with-hyphen:"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "name-with-hyphen",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":name-with-hyphen:",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Set);

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
    fn unset_prefix() {
        let mi =
            crate::document::Attribute::parse(crate::Span::new(":!foo:\nblah"), &Parser::default())
                .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 3,
                    offset: 2,
                },
                value_source: None,
                value: InterpretedValue::Unset,
                source: Span {
                    data: ":!foo:",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Unset);

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 2,
                col: 1,
                offset: 7
            }
        );
    }

    #[test]
    fn unset_postfix() {
        let mi =
            crate::document::Attribute::parse(crate::Span::new(":foo!:\nblah"), &Parser::default())
                .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Unset,
                source: Span {
                    data: ":foo!:",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Unset);

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 2,
                col: 1,
                offset: 7
            }
        );
    }

    #[test]
    fn err_unset_prefix_and_postfix() {
        assert!(
            crate::document::Attribute::parse(
                crate::Span::new(":!foo!:\nblah"),
                &Parser::default()
            )
            .is_none()
        );
    }

    #[test]
    fn err_invalid_ident1() {
        assert!(
            crate::document::Attribute::parse(
                crate::Span::new(":@invalid:\nblah"),
                &Parser::default()
            )
            .is_none()
        );
    }

    #[test]
    fn err_missing_closing_colon() {
        // A valid attribute name that is not followed by a closing colon is not
        // an attribute entry.
        assert!(
            crate::document::Attribute::parse(
                crate::Span::new(":foo bar\nblah"),
                &Parser::default()
            )
            .is_none()
        );

        // ... including at end of input.
        assert!(
            crate::document::Attribute::parse(crate::Span::new(":foo"), &Parser::default())
                .is_none()
        );
    }

    #[test]
    fn name_captures_trailing_non_word_char() {
        // A name may contain (and end with) characters that are not valid in a
        // canonical attribute name; the raw name runs up to the closing colon.
        // The stray characters are dropped later, when the name is sanitized
        // into an attribute key (`invalid@` becomes `invalid`).
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":invalid@:\nblah"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "invalid@",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":invalid@:",
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
                line: 2,
                col: 1,
                offset: 11
            }
        );
    }

    #[test]
    fn name_with_spaces() {
        // A name containing spaces is captured verbatim up to the closing colon.
        // It is sanitized to `authorinitials` before it is stored as an
        // attribute (see the parser's `remap_attr_name`); here we only check
        // that the entry parses and preserves the raw name span.
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":Author Initials: SJR"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "Author Initials",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "SJR",
                    line: 1,
                    col: 19,
                    offset: 18,
                }),
                value: InterpretedValue::Value("SJR"),
                source: Span {
                    data: ":Author Initials: SJR",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );
    }

    #[test]
    fn err_invalid_ident3() {
        assert!(
            crate::document::Attribute::parse(
                crate::Span::new(":-invalid:\nblah"),
                &Parser::default()
            )
            .is_none()
        );
    }

    #[test]
    fn value_with_soft_wrap() {
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":foo: bar \\\n blah"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "bar \\\n blah",
                    line: 1,
                    col: 7,
                    offset: 6,
                }),
                value: InterpretedValue::Value("bar blah"),
                source: Span {
                    data: ":foo: bar \\\n blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar blah"));

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 2,
                col: 6,
                offset: 17
            }
        );
    }

    #[test]
    fn bare_trailing_backslash_is_literal() {
        // A bare trailing backslash (no preceding space) is not a soft-wrap line
        // continuation; it is a literal character and the value ends at that line.
        // See https://github.com/asciidoc-rs/asciidoc-parser/issues/666.
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":longpath: very/long/path/to/some/\\\nsubdirectory"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "longpath",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "very/long/path/to/some/\\",
                    line: 1,
                    col: 12,
                    offset: 11,
                }),
                value: InterpretedValue::Value("very/long/path/to/some/\\"),
                source: Span {
                    data: ":longpath: very/long/path/to/some/\\",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.item.value(),
            InterpretedValue::Value("very/long/path/to/some/\\")
        );

        // `subdirectory` is left as a separate line, not folded into the value.
        assert_eq!(
            mi.after,
            Span {
                data: "subdirectory",
                line: 2,
                col: 1,
                offset: 36
            }
        );
    }

    #[test]
    fn literal_trailing_backslash_on_final_line() {
        // The soft-wrap continuation on the first line is folded, but the bare
        // trailing backslash on the final line is kept as a literal character.
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":foo: bar \\\nbaz\\"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar baz\\"));
    }

    #[test]
    fn value_with_hard_wrap() {
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":foo: bar + \\\n blah"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "bar + \\\n blah",
                    line: 1,
                    col: 7,
                    offset: 6,
                }),
                value: InterpretedValue::Value("bar +\nblah"),
                source: Span {
                    data: ":foo: bar + \\\n blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar +\nblah"));

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 2,
                col: 6,
                offset: 19
            }
        );
    }

    #[test]
    fn single_line_trailing_hard_break_marker_is_stripped() {
        // A single-line value ending in a legacy continuation marker (a space
        // followed by a single `+`) with no following non-blank line has that
        // marker stripped from the interpreted value, matching Asciidoctor. The
        // raw `value_source` still contains the literal ` +`. Contrast with
        // `legacy_multi_line_value_is_fused`, where a following line _is_ folded
        // in.
        let mi =
            crate::document::Attribute::parse(crate::Span::new(":foo: bar +"), &Parser::default())
                .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "bar +",
                    line: 1,
                    col: 7,
                    offset: 6,
                }),
                value: InterpretedValue::Value("bar"),
                source: Span {
                    data: ":foo: bar +",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar"));

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 12,
                offset: 11
            }
        );
    }

    #[test]
    fn legacy_multi_line_value_is_fused() {
        // A legacy `+`-continued value fuses the following line: the trailing
        // ` +` marker is stripped and the lines are folded with a space (the
        // value does not end in a hard line break marker after the strip). The
        // following line is consumed, so `after` is empty. See #729.
        let mi = crate::document::Attribute::parse(
            crate::Span::new(":foo: bar +\nblah"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "foo",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "bar +\nblah",
                    line: 1,
                    col: 7,
                    offset: 6,
                }),
                value: InterpretedValue::Value("bar blah"),
                source: Span {
                    data: ":foo: bar +\nblah",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar blah"));

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 2,
                col: 5,
                offset: 16
            }
        );
    }

    #[test]
    fn legacy_hard_break_marker_edge_cases() {
        // The legacy continuation marker is exactly a space followed by a single
        // `+`. When a following non-blank line is present, the marker fuses the
        // lines (stripping the marker and right-trimming). Contrast with markers
        // that are _not_ legacy continuations (`++`, a bare `+`, a tab before the
        // `+`, or a lone `+`), which leave the value on a single line.
        let value = |src| {
            crate::document::Attribute::parse(crate::Span::new(src), &Parser::default())
                .unwrap()
                .item
                .value()
                .clone()
        };

        // Extra space(s) before the `+` are trimmed away with the marker, and the
        // following line is folded in with a single space.
        assert_eq!(value(":foo: bar  +\nx"), InterpretedValue::Value("bar x"));

        // A tab preceding the marker's space is also trimmed.
        assert_eq!(value(":foo: bar\t +\nx"), InterpretedValue::Value("bar x"));

        // `++` is not a continuation marker; the value stays on one line verbatim.
        assert_eq!(value(":foo: bar ++\nx"), InterpretedValue::Value("bar ++"));

        // A `+` with no preceding space is a literal character (no continuation).
        assert_eq!(value(":foo: bar+\nx"), InterpretedValue::Value("bar+"));

        // A tab (rather than a space) before the `+` does not form a marker.
        assert_eq!(value(":foo: bar\t+\nx"), InterpretedValue::Value("bar\t+"));

        // A lone `+` (nothing before it) is not a marker; it is preserved.
        assert_eq!(value(":foo: +\nx"), InterpretedValue::Value("+"));

        // An earlier ` +` in the fused first line ends in a hard line break
        // marker, so the following line is joined with a newline rather than a
        // space.
        assert_eq!(
            value(":foo: bar + +\nx"),
            InterpretedValue::Value("bar +\nx")
        );

        // Right-trimming after the marker uses ASCII whitespace rules (Ruby
        // `rstrip`); a preceding non-breaking space is significant and preserved.
        assert_eq!(
            value(":foo: bar\u{00a0} +\nx"),
            InterpretedValue::Value("bar\u{00a0} x")
        );
    }

    #[test]
    fn is_block() {
        let mut parser = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(":foo: bar\nblah"), &mut parser);

        let mi = maw.item.unwrap();
        let block = mi.item;

        assert_eq!(
            block,
            Block::DocumentAttribute(Attribute {
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
            })
        );

        assert_eq!(block.content_model(), ContentModel::Empty);
        assert!(block.rendered_content().is_none());
        assert_eq!(block.raw_context().deref(), "attribute");
        assert!(block.nested_blocks().next().is_none());
        assert!(block.title_source().is_none());
        assert!(block.title().is_none());
        assert!(block.anchor().is_none());
        assert!(block.anchor_reftext().is_none());
        assert!(block.attrlist().is_none());
        assert_eq!(block.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            block.span(),
            Span {
                data: ":foo: bar",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        let crate::blocks::Block::DocumentAttribute(attr) = block else {
            panic!("Wrong type");
        };

        assert_eq!(attr.value(), InterpretedValue::Value("bar"));

        assert_eq!(
            mi.after,
            Span {
                data: "blah",
                line: 2,
                col: 1,
                offset: 10
            }
        );
    }

    #[test]
    fn affects_document_state() {
        let mut parser = Parser::default().with_intrinsic_attribute(
            "agreed",
            "yes",
            ModificationContext::Anywhere,
        );

        let doc =
            parser.parse("We are agreed? {agreed}\n\n:agreed: no\n\nAre we still agreed? {agreed}");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "We are agreed? {agreed}",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "We are agreed? yes",
                },
                source: Span {
                    data: "We are agreed? {agreed}",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );

        let _ = blocks.next().unwrap();

        let block3 = blocks.next().unwrap();

        assert_eq!(
            block3,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Are we still agreed? {agreed}",
                        line: 5,
                        col: 1,
                        offset: 38,
                    },
                    rendered: "Are we still agreed? no",
                },
                source: Span {
                    data: "Are we still agreed? {agreed}",
                    line: 5,
                    col: 1,
                    offset: 38,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );

        let mut warnings = doc.warnings();
        assert!(warnings.next().is_none());
    }

    #[test]
    fn block_enforces_permission() {
        let mut parser = Parser::default().with_intrinsic_attribute(
            "agreed",
            "yes",
            ModificationContext::ApiOnly,
        );

        let doc = parser.parse("Hello\n\n:agreed: no\n\nAre we agreed? {agreed}");

        let mut blocks = doc.nested_blocks();
        let _ = blocks.next().unwrap();
        let _ = blocks.next().unwrap();
        let block3 = blocks.next().unwrap();

        assert_eq!(
            block3,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Are we agreed? {agreed}",
                        line: 5,
                        col: 1,
                        offset: 20,
                    },
                    rendered: "Are we agreed? yes",
                },
                source: Span {
                    data: "Are we agreed? {agreed}",
                    line: 5,
                    col: 1,
                    offset: 20,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );

        let mut warnings = doc.warnings();
        let warning1 = warnings.next().unwrap();

        assert_eq!(
            &warning1.source,
            Span {
                data: ":agreed: no",
                line: 3,
                col: 1,
                offset: 7,
            }
        );

        assert_eq!(
            warning1.warning,
            WarningType::AttributeValueIsLocked("agreed".to_owned(),)
        );

        assert!(warnings.next().is_none());
    }

    mod fold_continuation_value {
        use super::super::fold_continuation_value;

        #[test]
        fn no_marker_returns_none() {
            // A plain single-line value has no continuation marker and needs no
            // folding.
            assert_eq!(fold_continuation_value("bar"), None);
            assert_eq!(fold_continuation_value("bar+"), None);
            assert_eq!(fold_continuation_value("bar ++"), None);
        }

        #[test]
        fn modern_soft_wrap_folds_with_space() {
            assert_eq!(
                fold_continuation_value("bar \\\nblah"),
                Some("bar blah".to_string())
            );
        }

        #[test]
        fn legacy_marker_folds_with_space() {
            assert_eq!(
                fold_continuation_value("This is the first +\nRuby implementation of +\nAsciiDoc."),
                Some("This is the first Ruby implementation of AsciiDoc.".to_string())
            );
        }

        #[test]
        fn hard_line_break_joins_with_newline() {
            // A soft-wrap value whose text ends in a hard line break marker
            // (` +`) is joined with a newline rather than a space.
            assert_eq!(
                fold_continuation_value("bar + \\\nblah"),
                Some("bar +\nblah".to_string())
            );
        }

        #[test]
        fn blank_line_terminates_value() {
            // A blank line inside the (already-extent-trimmed) data terminates
            // the fold without consuming further lines.
            assert_eq!(fold_continuation_value("a +\n\nb"), Some("a".to_string()));
        }

        #[test]
        fn crlf_line_endings_are_normalized() {
            assert_eq!(
                fold_continuation_value("bar +\r\nblah"),
                Some("bar blah".to_string())
            );
        }
    }

    mod interpreted_value {
        mod impl_debug {
            use crate::document::InterpretedValue;

            #[test]
            fn value_empty_string() {
                let interpreted_value = InterpretedValue::Value("".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Value(\"\")");
            }

            #[test]
            fn value_simple_string() {
                let interpreted_value = InterpretedValue::Value("hello".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Value(\"hello\")");
            }

            #[test]
            fn value_string_with_spaces() {
                let interpreted_value = InterpretedValue::Value("hello world".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Value(\"hello world\")");
            }

            #[test]
            fn value_string_with_special_chars() {
                let interpreted_value = InterpretedValue::Value("test!@#$%^&*()".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Value(\"test!@#$%^&*()\")");
            }

            #[test]
            fn value_string_with_quotes() {
                let interpreted_value = InterpretedValue::Value("value\"with'quotes".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(
                    debug_output,
                    "InterpretedValue::Value(\"value\\\"with'quotes\")"
                );
            }

            #[test]
            fn value_string_with_newlines() {
                let interpreted_value = InterpretedValue::Value("line1\nline2\nline3".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(
                    debug_output,
                    "InterpretedValue::Value(\"line1\\nline2\\nline3\")"
                );
            }

            #[test]
            fn value_string_with_backslashes() {
                let interpreted_value = InterpretedValue::Value("path\\to\\file".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(
                    debug_output,
                    "InterpretedValue::Value(\"path\\\\to\\\\file\")"
                );
            }

            #[test]
            fn value_string_with_unicode() {
                let interpreted_value = InterpretedValue::Value("café 🚀 ñoño".to_string());
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Value(\"café 🚀 ñoño\")");
            }

            #[test]
            fn set() {
                let interpreted_value = InterpretedValue::Set;
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Set");
            }

            #[test]
            fn unset() {
                let interpreted_value = InterpretedValue::Unset;
                let debug_output = format!("{:?}", interpreted_value);
                assert_eq!(debug_output, "InterpretedValue::Unset");
            }
        }
    }
}
