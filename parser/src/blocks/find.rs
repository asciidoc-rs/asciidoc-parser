//! A search API for locating blocks within a parsed document.
//!
//! The public entry point is the [`FindBlocks`] trait; see its documentation
//! for an overview and examples.

use std::{borrow::Cow, slice::Iter};

use crate::{
    Document,
    blocks::{Block, IsBlock, TableCellContent},
};

/// A search over a [`Document`] or a [`Block`] for its descendant blocks.
///
/// This is the Rust-native counterpart to Asciidoctor's [block-finding API].
/// Where Asciidoctor exposes `find_by(selector = {}, &block_filter)`, this
/// trait leans on Rust's iterator patterns: [`descendant_blocks`] is a plain
/// [`Iterator`] you compose with the standard combinators, [`find_blocks`]
/// takes a declarative [`BlockSelector`], and [`traverse_blocks`] gives
/// per-block control over the walk (including subtree pruning) via the
/// [`Descend`] enum.
///
/// The trait is implemented for [`Document`] and [`Block`] and is sealed (it
/// cannot be implemented for other types); bring it into scope to call its
/// methods. All of the returned iterators yield `&Block` in document order and
/// never yield the receiver itself.
///
/// # Traversal model
///
/// The walk is depth-first and yields blocks in **document order**. It reaches
/// every block reachable through the tree, including the children of
/// Markdown-style blockquotes (whose children are not exposed through the
/// `'src`-bound [`IsBlock::nested_blocks`] because they borrow the block's own
/// owned source). AsciiDoc table cells are separate nested documents and are
/// **not** entered by default; opt in with
/// [`BlockSelector::traverse_documents`] (the analog of Asciidoctor's
/// `traverse_documents` selector key).
///
/// # Differences from Asciidoctor
///
/// * **The receiver is never yielded.** These iterators visit descendants only.
///   (Asciidoctor includes the receiver as a candidate.) A [`Document`] is not
///   a [`Block`], so "descendants only" is the one rule that reads the same on
///   both receivers. To include a starting block, chain it yourself:
///   `std::iter::once(block).chain(block.descendant_blocks())`.
/// * **`traverse_documents` is off by default** (as in Asciidoctor).
///
/// # Examples
///
/// ```
/// use asciidoc_parser::{
///     Parser,
///     blocks::{Block, BlockSelector, Descend, FindBlocks, IsBlock},
/// };
///
/// let doc =
///     Parser::default().parse("= Title\n\n== First\n\n[source,rust]\n----\nfn main() {}\n----\n");
///
/// // The Rust-native core: a plain iterator you compose with std combinators.
/// let sections = doc
///     .descendant_blocks()
///     .filter(|b| matches!(b, Block::Section(_)))
///     .count();
/// assert_eq!(sections, 1);
///
/// // A declarative selector, like `find_by(context: :listing, style: 'source')`.
/// let listings: Vec<_> = doc
///     .find_blocks(&BlockSelector::new().context("listing").style("source"))
///     .collect();
/// assert_eq!(listings.len(), 1);
///
/// // Per-block traversal control, like the Asciidoctor block filter. Collect
/// // only top-level sidebars: include each sidebar but do not descend into it,
/// // and reject everything else (so nested sidebars are not reported).
/// let top_sidebars: Vec<_> = doc
///     .traverse_blocks(|b| {
///         if b.resolved_context().as_ref() == "sidebar" {
///             Descend::Prune
///         } else {
///             Descend::Reject
///         }
///     })
///     .collect();
/// assert!(top_sidebars.is_empty());
/// ```
///
/// [block-finding API]: https://docs.asciidoctor.org/asciidoctor/latest/api/find-blocks/
/// [`descendant_blocks`]: Self::descendant_blocks
/// [`find_blocks`]: Self::find_blocks
/// [`traverse_blocks`]: Self::traverse_blocks
pub trait FindBlocks<'a>: sealed::Sealed<'a> {
    /// Returns a depth-first, document-order iterator over every descendant
    /// block.
    ///
    /// This is the equivalent of Asciidoctor's `find_by` with no arguments.
    /// Because it is an ordinary [`Iterator`], the idiomatic way to search is
    /// to compose it with the standard combinators (`filter`, `find`,
    /// `map`, …).
    ///
    /// AsciiDoc table cells are not entered; use [`find_blocks`] with
    /// [`BlockSelector::traverse_documents`] to include them.
    ///
    /// [`find_blocks`]: Self::find_blocks
    fn descendant_blocks(&'a self) -> Descendants<'a> {
        Descendants {
            stack: vec![self.seed_children(false)],
            traverse_documents: false,
        }
    }

    /// Returns an iterator over the descendant blocks that match `selector`.
    ///
    /// This mirrors Asciidoctor's `find_by(selector)`: a block is yielded when
    /// it matches every field the selector sets (see [`BlockSelector`]).
    /// Traversal still descends through non-matching blocks, so matches at
    /// any depth are found.
    fn find_blocks(&'a self, selector: &BlockSelector<'a>) -> FindBlocksIter<'a> {
        FindBlocksIter {
            inner: Descendants {
                stack: vec![self.seed_children(selector.traverse_documents)],
                traverse_documents: selector.traverse_documents,
            },
            selector: selector.clone(),
        }
    }

    /// Returns the first descendant block whose [id](IsBlock::id) equals `id`,
    /// if any.
    ///
    /// Block ids are unique within a document, so at most one block can match.
    /// This is the equivalent of Asciidoctor's `find_by(id: '…').first`.
    fn find_block_by_id(&'a self, id: &str) -> Option<&'a Block<'a>> {
        self.descendant_blocks()
            .find(|block| block.id() == Some(id))
    }

    /// Returns an iterator that walks the descendant blocks under the control
    /// of `control`, which is called once per block, in document order, to
    /// decide whether the block is yielded and whether its children are
    /// visited.
    ///
    /// This is the equivalent of Asciidoctor's `find_by` block filter; the
    /// [`Descend`] return value plays the role of the filter's `:accept` /
    /// `:skip` / `:reject` / `:prune` symbols and is the mechanism for pruning
    /// whole subtrees. AsciiDoc table cells are not entered.
    fn traverse_blocks<F>(&'a self, control: F) -> TraverseBlocks<'a, F>
    where
        F: FnMut(&Block<'a>) -> Descend,
    {
        TraverseBlocks {
            stack: vec![self.seed_children(false)],
            control,
        }
    }
}

impl<'a> FindBlocks<'a> for Document<'a> {}
impl<'a> FindBlocks<'a> for Block<'a> {}

mod sealed {
    use super::{ChildBlocks, children_of};
    use crate::{Document, blocks::IsBlock};

    /// Seals [`FindBlocks`](super::FindBlocks) and supplies the traversal seed:
    /// the receiver's direct child blocks.
    pub trait Sealed<'a> {
        fn seed_children(&'a self, traverse_documents: bool) -> ChildBlocks<'a>;
    }

    impl<'a> Sealed<'a> for Document<'a> {
        fn seed_children(&'a self, _traverse_documents: bool) -> ChildBlocks<'a> {
            // A document's direct children are never table cells, so the flag
            // does not affect the seed; any tables among the children are
            // expanded with the flag during the walk itself.
            ChildBlocks::Slice(self.nested_blocks())
        }
    }

    impl<'a> Sealed<'a> for super::Block<'a> {
        fn seed_children(&'a self, traverse_documents: bool) -> ChildBlocks<'a> {
            children_of(self, traverse_documents)
        }
    }
}

/// A declarative selector for [`FindBlocks::find_blocks`], mirroring the
/// selector hash of Asciidoctor's `find_by`.
///
/// Build one with [`new`](Self::new) and the builder methods. A field that is
/// left unset matches any block; when several fields are set they are combined
/// with logical AND.
///
/// | Method | Matches against |
/// |---|---|
/// | [`context`](Self::context) | [`IsBlock::resolved_context`] |
/// | [`style`](Self::style) | [`IsBlock::declared_style`] (see below) |
/// | [`id`](Self::id) | [`IsBlock::id`] |
/// | [`role`](Self::role) | membership in [`IsBlock::roles`] |
///
/// [`style`](Self::style) matches [`declared_style`](IsBlock::declared_style),
/// which is the block's resolved style and tracks Asciidoctor's `style` in the
/// common cases — including variant blocks that report their style even in
/// shorthand form (e.g. an admonition written `NOTE:` matches `style("NOTE")`).
/// The one divergence: a style that masquerades as a built-in context (e.g.
/// `[example]`, `[sidebar]`) is promoted to the block's context, so match those
/// with [`context`](Self::context) rather than `style`.
///
/// ```
/// use asciidoc_parser::{
///     Parser,
///     blocks::{BlockSelector, FindBlocks},
/// };
///
/// let doc = Parser::default().parse("[#intro]\nHello.\n");
/// let block = doc.find_blocks(&BlockSelector::new().id("intro")).next();
/// assert!(block.is_some());
/// ```
#[derive(Clone, Debug, Default)]
pub struct BlockSelector<'a> {
    context: Option<Cow<'a, str>>,
    style: Option<Cow<'a, str>>,
    id: Option<Cow<'a, str>>,
    role: Option<Cow<'a, str>>,
    traverse_documents: bool,
}

impl<'a> BlockSelector<'a> {
    /// Creates a selector that matches every block.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts the match to blocks whose
    /// [resolved context](IsBlock::resolved_context) equals `context` (e.g.
    /// `"listing"`, `"section"`, `"sidebar"`).
    pub fn context(mut self, context: impl Into<Cow<'a, str>>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Restricts the match to blocks whose
    /// [declared style](IsBlock::declared_style) equals `style` (e.g.
    /// `"source"`, `"verse"`, `"NOTE"`).
    pub fn style(mut self, style: impl Into<Cow<'a, str>>) -> Self {
        self.style = Some(style.into());
        self
    }

    /// Restricts the match to the block whose [id](IsBlock::id) equals `id`.
    pub fn id(mut self, id: impl Into<Cow<'a, str>>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Restricts the match to blocks that carry `role` among their
    /// [roles](IsBlock::roles).
    pub fn role(mut self, role: impl Into<Cow<'a, str>>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Sets whether the traversal descends into AsciiDoc table cells (nested
    /// documents). Off by default.
    pub fn traverse_documents(mut self, traverse_documents: bool) -> Self {
        self.traverse_documents = traverse_documents;
        self
    }

    /// Returns `true` if `block` satisfies every field this selector sets.
    ///
    /// This does not consider [`traverse_documents`](Self::traverse_documents),
    /// which governs traversal rather than whether an individual block matches.
    pub fn matches(&self, block: &Block<'_>) -> bool {
        if let Some(context) = &self.context
            && block.resolved_context().as_ref() != context.as_ref()
        {
            return false;
        }

        if let Some(style) = &self.style
            && block.declared_style() != Some(style.as_ref())
        {
            return false;
        }

        if let Some(id) = &self.id
            && block.id() != Some(id.as_ref())
        {
            return false;
        }

        if let Some(role) = &self.role
            && !block.roles().iter().any(|r| *r == role.as_ref())
        {
            return false;
        }

        true
    }
}

/// The disposition of a block during a [`traverse_blocks`] walk: whether the
/// block is included in the results and whether its children are visited.
///
/// The variants correspond one-to-one with the return values of Asciidoctor's
/// `find_by` block filter.
///
/// [`traverse_blocks`]: FindBlocks::traverse_blocks
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Descend {
    /// Include this block and descend into its children. (Asciidoctor `:accept`
    /// / `true`.)
    Accept,

    /// Omit this block but still descend into its children. (Asciidoctor
    /// `:skip` / `false`.)
    Skip,

    /// Omit this block and skip its children. (Asciidoctor `:reject`.)
    Reject,

    /// Include this block but skip its children. (Asciidoctor `:prune`.)
    Prune,
}

/// A depth-first, document-order iterator over descendant blocks, returned by
/// [`FindBlocks::descendant_blocks`].
pub struct Descendants<'a> {
    stack: Vec<ChildBlocks<'a>>,
    traverse_documents: bool,
}

impl<'a> Iterator for Descendants<'a> {
    type Item = &'a Block<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.stack.last_mut()?.next() {
                None => {
                    self.stack.pop();
                }
                Some(block) => {
                    self.stack.push(children_of(block, self.traverse_documents));
                    return Some(block);
                }
            }
        }
    }
}

/// An iterator over the descendant blocks matching a [`BlockSelector`],
/// returned by [`FindBlocks::find_blocks`].
pub struct FindBlocksIter<'a> {
    inner: Descendants<'a>,
    selector: BlockSelector<'a>,
}

impl<'a> Iterator for FindBlocksIter<'a> {
    type Item = &'a Block<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .by_ref()
            .find(|block| self.selector.matches(block))
    }
}

/// An iterator that walks descendant blocks under the control of a closure,
/// returned by [`FindBlocks::traverse_blocks`].
pub struct TraverseBlocks<'a, F> {
    stack: Vec<ChildBlocks<'a>>,
    control: F,
}

impl<'a, F> Iterator for TraverseBlocks<'a, F>
where
    F: FnMut(&Block<'a>) -> Descend,
{
    type Item = &'a Block<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let block = match self.stack.last_mut()?.next() {
                None => {
                    self.stack.pop();
                    continue;
                }
                Some(block) => block,
            };

            match (self.control)(block) {
                Descend::Accept => {
                    self.stack.push(children_of(block, false));
                    return Some(block);
                }
                Descend::Skip => {
                    self.stack.push(children_of(block, false));
                }
                Descend::Prune => return Some(block),
                Descend::Reject => {}
            }
        }
    }
}

/// An iterator over the direct child blocks of a single node.
///
/// This is the traversal frame the depth-first walkers push and pop. It is not
/// part of the public API surface: it appears only in the signature of the
/// sealed `Sealed` trait (hence `pub` to satisfy the privacy lint), and the
/// enclosing `find` module is private, so it cannot be named from outside this
/// crate.
#[doc(hidden)]
pub enum ChildBlocks<'a> {
    /// Children stored contiguously in a slice (the common case).
    Slice(Iter<'a, Block<'a>>),

    /// Children gathered across a table's AsciiDoc cells, boxed because the
    /// flattened iterator type cannot be named. Allocated only when actually
    /// descending into a table with `traverse_documents` enabled.
    Boxed(Box<dyn Iterator<Item = &'a Block<'a>> + 'a>),

    /// No children.
    Empty,
}

impl<'a> Iterator for ChildBlocks<'a> {
    type Item = &'a Block<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ChildBlocks::Slice(iter) => iter.next(),
            ChildBlocks::Boxed(iter) => iter.next(),
            ChildBlocks::Empty => None,
        }
    }
}

/// Returns the direct child blocks of `block`, in document order.
///
/// This is the one place that knows how to reach each block type's children,
/// including the two kinds that the `'src`-bound [`IsBlock::nested_blocks`]
/// cannot expose:
///
/// * A Markdown-style blockquote's children borrow the block's own owned
///   source, so they are read through [`QuoteBlock::blocks`] rather than
///   `nested_blocks`. (For a `____`-delimited quote the two agree.)
/// * An AsciiDoc table cell is a nested document; its blocks are reached only
///   when `traverse_documents` is set.
///
/// [`QuoteBlock::blocks`]: crate::blocks::QuoteBlock::blocks
fn children_of<'a>(block: &'a Block<'a>, traverse_documents: bool) -> ChildBlocks<'a> {
    match block {
        Block::Quote(quote) => ChildBlocks::Slice(quote.blocks().iter()),

        Block::Table(table) => {
            if traverse_documents {
                let blocks = table
                    .header_row()
                    .into_iter()
                    .chain(table.body_rows().iter())
                    .chain(table.footer_row())
                    .flat_map(|row| row.cells().iter())
                    .filter_map(|cell| match cell.content() {
                        TableCellContent::AsciiDoc(cell) => Some(cell.blocks().iter()),
                        TableCellContent::Simple(_) => None,
                    })
                    .flatten();

                ChildBlocks::Boxed(Box::new(blocks))
            } else {
                ChildBlocks::Empty
            }
        }

        other => ChildBlocks::Slice(other.nested_blocks()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{
        blocks::{Block, BlockSelector, Descend, FindBlocks, IsBlock},
        tests::prelude::*,
    };

    /// Collects the resolved context of every descendant block, in order.
    fn contexts<'a, T: FindBlocks<'a>>(node: &'a T) -> Vec<String> {
        node.descendant_blocks()
            .map(|b| b.resolved_context().as_ref().to_string())
            .collect()
    }

    #[test]
    fn empty_document_has_no_descendants() {
        let doc = Parser::default().parse("");
        assert_eq!(doc.descendant_blocks().count(), 0);
        assert!(doc.find_block_by_id("anything").is_none());
    }

    #[test]
    fn document_order_and_nesting() {
        // A section containing a list (which owns its items) and a source
        // listing. The walk should yield each in document order, descending into
        // the section and the list.
        let doc = Parser::default()
            .parse("== Section\n\n* one\n* two\n\n[source,rust]\n----\nfn main() {}\n----\n");

        let contexts = contexts(&doc);

        // section, then the list, then each item and the paragraph it holds,
        // then the listing.
        assert_eq!(
            contexts,
            vec![
                "section",
                "list",
                "list_item",
                "paragraph",
                "list_item",
                "paragraph",
                "listing"
            ]
        );
    }

    #[test]
    fn descendant_blocks_compose_with_std_combinators() {
        let doc = Parser::default().parse("== A\n\ntext\n\n== B\n\ntext\n");

        let sections = doc
            .descendant_blocks()
            .filter(|b| matches!(b, Block::Section(_)))
            .count();

        assert_eq!(sections, 2);
    }

    #[test]
    fn find_blocks_by_context_and_style() {
        let doc = Parser::default()
            .parse("[source,rust]\n----\nfn main() {}\n----\n\n----\nplain listing\n----\n");

        // Both are listings.
        assert_eq!(
            doc.find_blocks(&BlockSelector::new().context("listing"))
                .count(),
            2
        );

        // Only one is a source listing.
        let sources: Vec<_> = doc
            .find_blocks(&BlockSelector::new().context("listing").style("source"))
            .collect();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources.first().unwrap().declared_style(), Some("source"));
    }

    #[test]
    fn find_blocks_by_role() {
        let doc = Parser::default().parse("[.important]\nAttention.\n\nOrdinary.\n");

        let matched: Vec<_> = doc
            .find_blocks(&BlockSelector::new().role("important"))
            .collect();

        assert_eq!(matched.len(), 1);
        assert!(matched.first().unwrap().roles().contains(&"important"));
    }

    #[test]
    fn find_block_by_id_hit_and_miss() {
        let doc = Parser::default().parse("[#intro]\nHello.\n\nWorld.\n");

        let intro = doc.find_block_by_id("intro").unwrap();
        assert_eq!(intro.id(), Some("intro"));
        assert!(doc.find_block_by_id("missing").is_none());
    }

    #[test]
    fn traverse_blocks_accept_yields_everything() {
        let doc = Parser::default().parse("== S\n\n* one\n* two\n");

        let all: Vec<_> = doc.traverse_blocks(|_| Descend::Accept).collect();
        let plain: Vec<_> = doc.descendant_blocks().collect();

        assert_eq!(all, plain);
    }

    #[test]
    fn traverse_blocks_prune_stops_at_matched_subtree() {
        // A sidebar nested inside a sidebar. Prune on the outer sidebar collects
        // it but not the inner one; reject everything else.
        let doc = Parser::default().parse("****\nOuter.\n\n[.inner]\n*****\nInner.\n*****\n****\n");

        let sidebars: Vec<_> = doc
            .traverse_blocks(|b| {
                if b.resolved_context().as_ref() == "sidebar" {
                    Descend::Prune
                } else {
                    Descend::Reject
                }
            })
            .collect();

        assert_eq!(sidebars.len(), 1);
    }

    #[test]
    fn traverse_blocks_skip_excludes_block_but_visits_children() {
        let doc = Parser::default().parse("****\nInside sidebar.\n****\n");

        // Skip the sidebar itself but descend into it: we should see the inner
        // paragraph but not the sidebar.
        let contexts: Vec<_> = doc
            .traverse_blocks(|b| {
                if b.resolved_context().as_ref() == "sidebar" {
                    Descend::Skip
                } else {
                    Descend::Accept
                }
            })
            .map(|b| b.resolved_context().as_ref().to_string())
            .collect();

        assert_eq!(contexts, vec!["paragraph"]);
    }

    #[test]
    fn markdown_quote_children_are_reached() {
        // Regression: a Markdown-style blockquote's children borrow the block's
        // own owned source and are invisible to `nested_blocks()`, but the search
        // API must still reach them (via `QuoteBlock::blocks()`).
        let doc = Parser::default().parse("> A quoted paragraph.\n");

        let contexts = contexts(&doc);
        assert_eq!(contexts, vec!["quote", "paragraph"]);
    }

    #[test]
    fn table_cells_require_traverse_documents() {
        // An AsciiDoc (`a|`) cell whose content is its own nested document.
        let doc = Parser::default().parse("|===\na| Cell _text_.\n|===\n");

        // By default the walk stops at the table.
        assert_eq!(doc.find_blocks(&BlockSelector::new()).count(), 1);
        assert_eq!(
            doc.find_blocks(&BlockSelector::new().context("table"))
                .count(),
            1
        );

        // Opting in reaches the paragraph inside the cell.
        assert_eq!(
            doc.find_blocks(&BlockSelector::new().traverse_documents(true))
                .count(),
            2
        );
    }

    #[test]
    fn find_blocks_on_a_block_searches_its_subtree() {
        // The API is available on `Block`, not only `Document`.
        let doc = Parser::default().parse("== Section\n\n* one\n* two\n");

        let section = doc
            .descendant_blocks()
            .find(|b| matches!(b, Block::Section(_)))
            .unwrap();

        // The section's own descendants: the list, and each item with the
        // paragraph it holds.
        assert_eq!(
            contexts(section),
            vec!["list", "list_item", "paragraph", "list_item", "paragraph"]
        );
    }
}
