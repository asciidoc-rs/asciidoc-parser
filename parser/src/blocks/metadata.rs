use std::ops::{RangeFrom, RangeTo};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    blocks::simple::is_section_header,
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// `BlockMetadata` represents the common elements that can precede any block
/// type (such as title and attribute list). It is used internally to track
/// those values before the specific block type is fully formed.
#[derive(Debug)]
pub(crate) struct BlockMetadata<'src> {
    /// The block's raw title, if any.
    pub(crate) title_source: Option<Span<'src>>,

    /// The block's title, if any, retained as a [`Content`] so a
    /// cross-reference embedded in the title can be resolved once the
    /// catalog is complete (by the document-order title pass). Its rendered
    /// text is the block's title string.
    pub(crate) title: Option<Content<'src>>,

    /// The block's anchor, if any. The span does not include the opening or
    /// closing square brace pair, nor reftext if it exists.
    pub(crate) anchor: Option<Span<'src>>,

    /// The block anchor's reftext, if any. The span includes only the portion
    /// from the first comma to just inside the closing square brace pair.
    pub(crate) anchor_reftext: Option<Span<'src>>,

    /// The block's attribute list, if any.
    pub(crate) attrlist: Option<Attrlist<'src>>,

    /// The source span as understood when the block metadata was first
    /// encountered. Does not necessarily end at the end of the block.
    pub(crate) source: Span<'src>,

    /// The source span after reading the optional title and attribute list.
    /// This is the beginning of content for the specific block type.
    pub(crate) block_start: Span<'src>,
}

impl<'src> BlockMetadata<'src> {
    /// (For testing only) Parse the block metadata from a raw text constant.
    #[cfg(test)]
    pub(crate) fn new(data: &'src str) -> Self {
        let mut temp_parser = Parser::default();
        Self::parse(Span::new(data), &mut temp_parser).item
    }

    /// Parse the title and attribute list for a block, if any.
    pub(crate) fn parse(source: Span<'src>, parser: &mut Parser) -> MatchAndWarnings<'src, Self> {
        let mut warnings: Vec<Warning<'src>> = vec![];
        let source = source.discard_empty_lines();

        // Block metadata items (title, anchor, and attribute list) can appear in any
        // order. We loop through lines until we can't parse any more metadata
        // items.
        let mut title_source: Option<Span<'src>> = None;
        let mut anchor: Option<Span<'src>> = None;
        let mut reftext: Option<Span<'src>> = None;
        let mut attrlist: Option<Attrlist<'src>> = None;
        let mut block_start = source;

        loop {
            let original_block_start = block_start;

            // Try to parse a title.
            if title_source.is_none() {
                let maybe_title = block_start.take_normalized_line();
                if maybe_title.item.starts_with('.') && !maybe_title.item.starts_with("..") {
                    let title = maybe_title.item.discard(1);
                    if title.take_whitespace().item.is_empty() {
                        title_source = Some(title);
                        block_start = maybe_title.after;
                        continue;
                    }
                }
            }

            // Try to parse a block anchor. Consecutive block anchors are
            // permitted and the last one wins (`[[bar]]` / `[[foo]]` → `foo`),
            // matching Asciidoctor, which simply overwrites the running `id`
            // (and its reftext) for each anchor line. The earlier `anchor.is_none()`
            // guard is therefore intentionally gone: a later anchor overrides.
            {
                let mut anchor_maw = parse_maybe_block_anchor(block_start);

                // Collect any warnings from the anchor parsing (e.g., empty anchor).
                if !anchor_maw.warnings.is_empty() {
                    warnings.append(&mut anchor_maw.warnings);
                }

                if let Some(mi) = anchor_maw.item {
                    if let Some(comma_position) = mi.item.position(|c| c == ',')
                        && comma_position < mi.item.len() - 1
                    {
                        let anchor_span = mi.item.slice_to(RangeTo {
                            end: comma_position,
                        });
                        let reftext_span = mi.item.slice_from(RangeFrom {
                            start: comma_position + 1,
                        });

                        // Validate anchor name.
                        if anchor_span.is_xml_name() {
                            anchor = Some(anchor_span);
                            reftext = Some(reftext_span);
                            block_start = mi.after;
                        } else {
                            warnings.push(Warning {
                                source: anchor_span,
                                warning: WarningType::InvalidBlockAnchorName,
                                origin: None,
                            });
                        }
                    } else {
                        // Validate anchor name.
                        if mi.item.is_xml_name() {
                            anchor = Some(mi.item);

                            // A later plain anchor (`[[foo]]`) clears any reftext
                            // carried by an earlier `[[bar,text]]`, keeping the
                            // last-wins semantics consistent across both fields.
                            reftext = None;
                            block_start = mi.after;
                        } else {
                            warnings.push(Warning {
                                source: mi.item,
                                warning: WarningType::InvalidBlockAnchorName,
                                origin: None,
                            });
                        }
                    }

                    if block_start != original_block_start {
                        continue;
                    }
                }
            }

            // Try to parse an attribute list. A block may be preceded by more
            // than one attribute list line (optionally straddling the title);
            // Asciidoctor merges them into a single set of attributes, with a
            // later line winning on conflict and otherwise accumulating.
            if let Some(MatchAndWarnings {
                item:
                    MatchedItem {
                        item: attrlist_item,
                        after: new_block_start,
                    },
                warnings: mut attrlist_warnings,
            }) = parse_maybe_attrlist_line(block_start, parser)
            {
                if !attrlist_warnings.is_empty() {
                    warnings.append(&mut attrlist_warnings);
                }

                match attrlist {
                    Some(ref mut existing) => existing.merge_block_attribute_line(attrlist_item),
                    None => attrlist = Some(attrlist_item),
                }
                block_start = new_block_start;
                continue;
            }

            // A comment line (`//`) or comment block (`////`) sitting between
            // already-collected metadata and a *following section heading* is
            // transparent: the metadata transfers across the comment to that
            // section (e.g. `[[sub]]` / `// comment` / `=== Sub-section` gives
            // the section id `sub`, and `[role=…]` / `////…////` / `== Section`
            // gives the section the role). Asciidoctor skips comments while
            // gathering block metadata; this crate deliberately *retains*
            // comment blocks as ordinary blocks, so the transparency is scoped
            // to the section-transfer case the wider divergence would otherwise
            // break. A comment that a metadata line directly decorates (with no
            // following section) is therefore left in place for normal dispatch.
            //
            // This only applies once at least one metadata item has been
            // collected, so a standalone comment (with no preceding metadata) is
            // untouched.
            if !(title_source.is_none() && anchor.is_none() && attrlist.is_none())
                && let Some(after_comments) =
                    skip_comments_before_section(block_start, parser.level_offset())
            {
                block_start = after_comments;
                continue;
            }

            // No more metadata items found.
            break;
        }

        // Determine the block title. A `.Title` line takes precedence; failing
        // that, a `title=` attribute in the block's attribute list supplies the
        // title (Asciidoctor treats the two as equivalent).
        let title: Option<Content<'src>> = match title_source.as_ref() {
            Some(span) => {
                let mut content = Content::from(*span);
                SubstitutionGroup::Normal.apply(&mut content, parser, None);
                Some(content)
            }
            None => attrlist.as_ref().and_then(Attrlist::title_attribute).map(
                |(value, value_is_substituted)| {
                    // A single-quoted `title=` value already had the normal
                    // substitution group applied when the attribute list was
                    // parsed; substituting it again here would double-escape
                    // special characters (and re-process inline markup), so it is
                    // used verbatim. Any other form receives the title (normal)
                    // substitutions now, matching a `.Title` line. (Attribute
                    // references in the value were resolved when the attribute
                    // list was parsed, so they are not re-evaluated here.)
                    //
                    // A `title=` value's rendered text is anchored at the block's
                    // source (rather than at the attribute value, which is
                    // borrowed from `attrlist` and would outlive this borrow):
                    // any cross-reference in it is rendered to its fallback here
                    // and not re-resolved later, matching the pre-existing
                    // treatment of a substituted value.
                    let rendered = if value_is_substituted {
                        value.to_string()
                    } else {
                        let mut content = Content::from(Span::new(value));
                        SubstitutionGroup::Normal.apply(&mut content, parser, None);
                        content.rendered.into_string()
                    };
                    Content::from_filtered(source, rendered)
                },
            ),
        };

        MatchAndWarnings {
            item: Self {
                title_source,
                title,
                anchor,
                anchor_reftext: reftext,
                attrlist,
                source,
                block_start,
            },
            warnings,
        }
    }

    /// Returns the block's rendered title text, if any.
    #[cfg(test)]
    pub(crate) fn title_str(&self) -> Option<&str> {
        self.title.as_ref().map(Content::rendered_str)
    }

    /// Return `true` if title, anchor, and attrlist are all empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.title.is_none() && self.anchor.is_none() && self.attrlist.is_none()
    }

    /// Return `true` if this block metadata has either the `discrete` or
    /// `float` block style.
    ///
    /// When used in the context of a section heading, this indicates that the
    /// heading should not mark the start of a new section.
    pub(crate) fn is_discrete(&self) -> bool {
        if let Some(ref attrlist) = self.attrlist
            && let Some(block_style) = attrlist.block_style()
        {
            block_style == "discrete" || block_style == "float"
        } else {
            false
        }
    }
}

/// Looks past a run of comment lines (`//`) and comment blocks (`////`),
/// together with the blank lines around them, to a following section heading.
/// On success, returns the source span at that heading so already-collected
/// block metadata attaches to the section rather than to the intervening
/// comment.
///
/// Returns `None` unless at least one comment was skipped *and* the run lands
/// on a section heading — every other case (no comment, or a comment that a
/// metadata line directly decorates with no following section) leaves the
/// comment in place for normal block dispatch, preserving this crate's
/// retention of comment blocks as ordinary blocks.
fn skip_comments_before_section(source: Span<'_>, level_offset: i32) -> Option<Span<'_>> {
    let mut cursor = source;
    let mut skipped_any = false;

    loop {
        let probe = cursor.discard_empty_lines();
        if probe.is_empty() {
            return None;
        }

        let line = probe.take_normalized_line();
        let data = line.item.data();

        // A comment block (`////`, or a longer run of slashes) is consumed
        // through its matching closing delimiter — or to end of input if it is
        // never closed, matching Asciidoctor's `read_lines_until terminator`.
        if data.len() >= 4 && data.chars().all(|c| c == '/') {
            let mut next = line.after;
            while !next.is_empty() {
                let inner = next.take_normalized_line();
                next = inner.after;
                if inner.item.data() == data {
                    break;
                }
            }
            cursor = next;
            skipped_any = true;
            continue;
        }

        // A comment line begins with `//` but is not the `///` (or longer) run
        // that opens neither a comment line nor a valid comment block.
        if data.starts_with("//") && !data.starts_with("///") {
            cursor = line.after;
            skipped_any = true;
            continue;
        }

        // A non-comment line ends the run. The metadata transfers across the
        // skipped comments only when they precede a section heading.
        return (skipped_any && is_section_header(data, level_offset)).then_some(probe);
    }
}

fn parse_maybe_block_anchor(
    source: Span<'_>,
) -> MatchAndWarnings<'_, Option<MatchedItem<'_, Span<'_>>>> {
    if !source.starts_with("[[") {
        return MatchAndWarnings {
            item: None,
            warnings: vec![],
        };
    }

    let MatchedItem {
        item: line,
        after: block_start,
    } = source.take_normalized_line();

    if !line.ends_with("]]") {
        return MatchAndWarnings {
            item: None,
            warnings: vec![],
        };
    }

    // Drop opening and closing brace pairs now that we know they are there.
    let anchor_src = line.slice(2..line.len() - 2);
    if anchor_src.is_empty() {
        return MatchAndWarnings {
            item: None,
            warnings: vec![Warning {
                source: anchor_src,
                warning: WarningType::EmptyBlockAnchorName,
                origin: None,
            }],
        };
    }

    MatchAndWarnings {
        item: Some(MatchedItem {
            item: anchor_src,
            after: block_start,
        }),
        warnings: vec![],
    }
}

fn parse_maybe_attrlist_line<'src>(
    source: Span<'src>,
    parser: &Parser,
) -> Option<MatchAndWarnings<'src, MatchedItem<'src, Attrlist<'src>>>> {
    let first_char = source.chars().next()?;
    if first_char != '[' {
        return None;
    }

    let MatchedItem {
        item: line,
        after: block_start,
    } = source.take_normalized_line();

    if !line.ends_with(']') {
        return None;
    }

    // Drop opening and closing braces now that we know they are there.
    let attrlist_src = line.slice(1..line.len() - 1);

    if attrlist_src.starts_with(' ')
        || attrlist_src.starts_with('\t')
        || (attrlist_src.starts_with('[') && attrlist_src.ends_with(']'))
    {
        return None;
    }

    let MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: _,
        },
        warnings,
    } = Attrlist::parse(attrlist_src, parser, AttrlistContext::Block);

    Some(MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: block_start,
        },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::prelude::*;

    #[test]
    fn metadata_order_title_anchor_attrlist() {
        let input = ".My Title\n[[my-anchor]]\n[role=\"example\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title_str(), Some("My Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "my-anchor",
                line: 2,
                col: 3,
                offset: 12,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_title_attrlist() {
        let input = "[[another-anchor]]\n.Another Title\n[role=\"sidebar\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title_str(), Some("Another Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "another-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_attrlist_title_anchor() {
        let input = "[role=\"note\"]\n.Third Title\n[[third-anchor]]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title_str(), Some("Third Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "third-anchor",
                line: 3,
                col: 3,
                offset: 29,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_attrlist_title() {
        let input = "[[fourth-anchor]]\n[role=\"warning\"]\n.Fourth Title\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title_str(), Some("Fourth Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "fourth-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_title_attrlist_only() {
        let input = ".Just Title\n[role=\"tip\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title_str(), Some("Just Title"));
        assert!(metadata.anchor.is_none());
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_attrlist_only() {
        let input = "[[just-anchor]]\n[role=\"caution\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert!(metadata.title.is_none());
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "just-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_attrlist_anchor_only() {
        let input = "[role=\"important\"]\n[[attrlist-first]]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert!(metadata.title.is_none());
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "attrlist-first",
                line: 2,
                col: 3,
                offset: 21,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn title_does_not_extend_via_plus_syntax() {
        let doc: crate::Document<'_> =
            Parser::default().parse(".Title abc +\ndef\n****\nStuff > nonsense\n****");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 2,
                                col: 1,
                                offset: 13,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: ".Title abc +\ndef",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: Some(Span {
                            data: "Title abc +",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },),
                        title: Some("Title abc +",),
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Stuff > nonsense",
                                    line: 4,
                                    col: 1,
                                    offset: 22,
                                },
                                rendered: "Stuff &gt; nonsense",
                            },
                            source: Span {
                                data: "Stuff > nonsense",
                                line: 4,
                                col: 1,
                                offset: 22,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "sidebar",
                        source: Span {
                            data: "****\nStuff > nonsense\n****",
                            line: 3,
                            col: 1,
                            offset: 17,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: ".Title abc +\ndef\n****\nStuff > nonsense\n****",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    mod merge_attribute_lines {
        use crate::tests::prelude::*;

        #[test]
        fn two_non_conflicting_named() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[baz=qux]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn conflicting_named_later_wins() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[foo=qux]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "qux");

            // Only one `foo` attribute should remain.
            assert_eq!(
                attrlist
                    .attributes()
                    .filter(|a| a.name() == Some("foo"))
                    .count(),
                1
            );
        }

        #[test]
        fn lines_straddling_a_title() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[foo=bar]\n.My Title\n[baz=qux]\ncontent\n",
            );

            assert_eq!(metadata.title_str(), Some("My Title"));

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn combined_with_anchor() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[foo=bar]\n[[my-anchor]]\n[baz=qux]\ncontent\n",
            );

            assert_eq!(
                metadata.anchor.unwrap(),
                Span {
                    data: "my-anchor",
                    line: 2,
                    col: 3,
                    offset: 12,
                }
            );

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn block_style_later_wins() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[sidebar]\n[example]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "example");
        }

        #[test]
        fn shorthand_id_and_role_across_lines() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[#myid]\n[.myrole]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "myid");
            assert_eq!(attrlist.roles(), vec!["myrole"]);
        }

        #[test]
        fn shorthand_roles_accumulate() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[.role1]\n[.role2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.roles(), vec!["role1", "role2"]);
        }

        #[test]
        fn shorthand_id_later_wins() {
            let metadata = crate::blocks::metadata::BlockMetadata::new("[#id1]\n[#id2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "id2");
        }

        #[test]
        fn shorthand_options_accumulate() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[%opt1]\n[%opt2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.options(), vec!["opt1", "opt2"]);
        }

        #[test]
        fn second_positional_later_wins() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[quote,Author1]\n[quote,Author2]\ncontent\n",
            );

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "quote");
            assert_eq!(attrlist.nth_attribute(2).unwrap().value(), "Author2");
        }

        #[test]
        fn first_positional_only_on_later_line() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[sidebar]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "sidebar");
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
        }

        #[test]
        fn extra_positional_only_on_later_line() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[quote]\n[quote,Author]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "quote");
            assert_eq!(attrlist.nth_attribute(2).unwrap().value(), "Author");
        }

        #[test]
        fn three_lines_mixed_with_title() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[#id1]\n.Title\n[.r1.r2]\n[foo=bar]\ncontent\n",
            );

            assert_eq!(metadata.title_str(), Some("Title"));

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "id1");
            assert_eq!(attrlist.roles(), vec!["r1", "r2"]);
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
        }
    }

    mod title_attribute {
        use crate::tests::prelude::*;

        #[test]
        fn sets_title_from_attribute() {
            // A `title=` entry in a block's attribute list sets the block title,
            // equivalent to a `.Title` line.
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[title=\"My Title\"]\ncontent\n");

            assert_eq!(metadata.title_str(), Some("My Title"));

            // A title supplied through an attribute has no `.Title` source line.
            assert!(metadata.title_source.is_none());
        }

        #[test]
        fn dot_title_line_wins_over_attribute() {
            // When both a `.Title` line and a `title=` attribute are present, the
            // `.Title` line takes precedence.
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                ".Line Title\n[title=\"Attr Title\"]\ncontent\n",
            );

            assert_eq!(metadata.title_str(), Some("Line Title"));
            assert!(metadata.title_source.is_some());
        }

        #[test]
        fn applies_normal_substitutions() {
            // A double-quoted `title=` value is not substituted while the
            // attribute list is parsed, so the title (normal) substitutions run
            // here: `>` becomes `&gt;`.
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[title=\"a > b\"]\ncontent\n");

            assert_eq!(metadata.title_str(), Some("a &gt; b"));
        }

        #[test]
        fn single_quoted_value_is_not_double_substituted() {
            // A single-quoted value already had the normal substitutions applied
            // when the attribute list was parsed. Substituting it again would
            // double-escape the `>` to `&amp;gt;`; the title must instead render
            // `a &gt; b`, matching the double-quoted form.
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[title='a > b']\ncontent\n");

            assert_eq!(metadata.title_str(), Some("a &gt; b"));
        }

        #[test]
        fn single_quoted_value_preserves_inline_markup() {
            // A single-quoted value's inline markup is rendered once (at
            // attribute-list parse time) and must not be re-escaped when the
            // title is formed.
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[title='*bold*']\ncontent\n");

            assert_eq!(metadata.title_str(), Some("<strong>bold</strong>"));
        }

        #[test]
        fn empty_title_attribute_yields_empty_title() {
            // An explicitly empty `title=` sets an empty (but present) title,
            // mirroring `.{empty}`.
            let metadata = crate::blocks::metadata::BlockMetadata::new("[title=]\ncontent\n");

            assert_eq!(metadata.title_str(), Some(""));
        }

        #[test]
        fn resolves_attribute_references_in_value() {
            // Attribute references in a `title=` value are resolved (when the
            // attribute list is parsed), then the title substitutions render the
            // result.
            let doc =
                Parser::default().parse(":who: World\n\n[title=\"Hello {who}\"]\n====\nbody\n====");

            let block = doc.nested_blocks().next().unwrap();
            assert_eq!(block.title(), Some("Hello World"));
        }

        #[test]
        fn straddles_a_second_attribute_line() {
            // A `title=` attribute merges across multiple attribute lines just
            // like any other named attribute, so it still supplies the title when
            // it sits on a separate line from the block style.
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[title=\"Merged Title\"]\n[sidebar]\ncontent\n",
            );

            assert_eq!(metadata.title_str(), Some("Merged Title"));

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "sidebar");
        }
    }
}
