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
        let attr_line = source.take_line_with_continuation()?;
        let colon = attr_line.item.take_prefix(":")?;

        let mut unset = false;
        let line = if colon.after.starts_with('!') {
            unset = true;
            colon.after.slice_from(1..)
        } else {
            colon.after
        };

        let name = line.take_user_attr_name()?;

        let line = if name.after.starts_with('!') && !unset {
            unset = true;
            name.after.slice_from(1..)
        } else {
            name.after
        };

        let line = line.take_prefix(":")?;

        let (value, value_source) = if unset {
            // Ensure line is now empty except for comment.
            (InterpretedValue::Unset, None)
        } else if line.after.is_empty() {
            (InterpretedValue::Set, None)
        } else {
            let raw_value = line.after.take_whitespace();
            (
                InterpretedValue::from_raw_value(&raw_value.after, parser),
                Some(raw_value.after),
            )
        };

        let source = source.trim_remainder(attr_line.after);
        Some(MatchedItem {
            item: Self {
                name: name.item,
                value_source,
                value,
                source: source.trim_trailing_whitespace(),
            },
            after: attr_line.after,
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
        let data = raw_value.data();
        let mut content = Content::from(*raw_value);

        if data.contains('\n') {
            let lines: Vec<&str> = data.lines().collect();
            let last_count = lines.len() - 1;

            let value: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(count, line)| {
                    let line = if count > 0 {
                        line.trim_start_matches(' ')
                    } else {
                        line
                    };

                    let line = line
                        .trim_start_matches('\r')
                        .trim_end_matches(' ')
                        .trim_end_matches('\\')
                        .trim_end_matches(' ');

                    if line.ends_with('+') {
                        format!("{}\n", line.trim_end_matches('+').trim_end_matches(' '))
                    } else if count < last_count {
                        format!("{line} ")
                    } else {
                        line.to_string()
                    }
                })
                .collect();

            content.rendered = CowStr::Boxed(value.join("").into_boxed_str());
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

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{
        HasSpan,
        blocks::{ContentModel, IsBlock, SimpleBlockStyle},
        content::SubstitutionGroup,
        parser::ModificationContext,
        tests::prelude::*,
        warnings::WarningType,
    };

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
    fn err_invalid_ident2() {
        assert!(
            crate::document::Attribute::parse(
                crate::Span::new(":invalid@:\nblah"),
                &Parser::default()
            )
            .is_none()
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
                value: InterpretedValue::Value("bar\nblah"),
                source: Span {
                    data: ":foo: bar + \\\n blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("bar\nblah"));

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
