use std::fmt;

use crate::{
    HasSpan,
    blocks::{HorizontalAlignment, IsBlock, VerticalAlignment},
    tests::fixtures::{Span, attributes::Attrlist, content::Content},
};

#[derive(Eq, PartialEq)]
pub(crate) struct TableBlock {
    pub columns: &'static [TableColumn],
    pub header_row: Option<TableRow>,
    pub body_rows: &'static [TableRow],
    pub footer_row: Option<TableRow>,
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub caption: Option<&'static str>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
}

impl fmt::Debug for TableBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableBlock")
            .field("columns", &self.columns)
            .field("header_row", &self.header_row)
            .field("body_rows", &self.body_rows)
            .field("footer_row", &self.footer_row)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("caption", &self.caption)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TableColumn {
    pub width: usize,
    pub h_align: HorizontalAlignment,
    pub v_align: VerticalAlignment,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TableRow {
    pub cells: &'static [TableCell],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TableCell {
    pub content: Content,
}

impl<'src> PartialEq<crate::blocks::TableBlock<'src>> for TableBlock {
    fn eq(&self, other: &crate::blocks::TableBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<TableBlock> for crate::blocks::TableBlock<'_> {
    fn eq(&self, other: &TableBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn columns_eq(fixture: &[TableColumn], observed: &[crate::blocks::TableColumn]) -> bool {
    fixture.len() == observed.len()
        && fixture.iter().zip(observed.iter()).all(|(f, o)| {
            f.width == o.width() && f.h_align == o.h_align() && f.v_align == o.v_align()
        })
}

fn cells_eq(fixture: &[TableCell], observed: &[crate::blocks::TableCell]) -> bool {
    fixture.len() == observed.len()
        && fixture
            .iter()
            .zip(observed.iter())
            .all(|(f, o)| f.content == *o.content())
}

fn row_eq(fixture: &TableRow, observed: &crate::blocks::TableRow) -> bool {
    cells_eq(fixture.cells, observed.cells())
}

fn rows_eq(fixture: &[TableRow], observed: &[crate::blocks::TableRow]) -> bool {
    fixture.len() == observed.len()
        && fixture
            .iter()
            .zip(observed.iter())
            .all(|(f, o)| row_eq(f, o))
}

fn opt_row_eq(fixture: &Option<TableRow>, observed: Option<&crate::blocks::TableRow>) -> bool {
    match (fixture, observed) {
        (None, None) => true,
        (Some(f), Some(o)) => row_eq(f, o),
        _ => false,
    }
}

fn fixture_eq_observed(fixture: &TableBlock, observed: &crate::blocks::TableBlock) -> bool {
    if !columns_eq(fixture.columns, observed.columns()) {
        return false;
    }

    if !opt_row_eq(&fixture.header_row, observed.header_row()) {
        return false;
    }

    if !rows_eq(fixture.body_rows, observed.body_rows()) {
        return false;
    }

    if !opt_row_eq(&fixture.footer_row, observed.footer_row()) {
        return false;
    }

    if fixture.title_source.is_some() != observed.title_source().is_some() {
        return false;
    }

    if let Some(ref fixture_title_source) = fixture.title_source
        && let Some(ref observed_title_source) = observed.title_source()
        && fixture_title_source != observed_title_source
    {
        return false;
    }

    if fixture.title.is_some() != observed.title().is_some() {
        return false;
    }

    if let Some(ref fixture_title) = fixture.title
        && let Some(ref observed_title) = observed.title()
        && fixture_title != observed_title
    {
        return false;
    }

    if fixture.caption.is_some() != observed.caption().is_some() {
        return false;
    }

    if let Some(ref fixture_caption) = fixture.caption
        && let Some(ref observed_caption) = observed.caption()
        && fixture_caption != observed_caption
    {
        return false;
    }

    if fixture.anchor.is_some() != observed.anchor().is_some() {
        return false;
    }

    if let Some(ref fixture_anchor) = fixture.anchor
        && let Some(ref observed_anchor) = observed.anchor()
        && fixture_anchor != observed_anchor
    {
        return false;
    }

    if fixture.anchor_reftext.is_some() != observed.anchor_reftext().is_some() {
        return false;
    }

    if let Some(ref fixture_anchor_reftext) = fixture.anchor_reftext
        && let Some(ref observed_anchor_reftext) = observed.anchor_reftext()
        && fixture_anchor_reftext != observed_anchor_reftext
    {
        return false;
    }

    if fixture.attrlist.is_some() != observed.attrlist().is_some() {
        return false;
    }

    if let Some(ref fixture_attrlist) = fixture.attrlist
        && let Some(ref observed_attrlist) = observed.attrlist()
        && &fixture_attrlist != observed_attrlist
    {
        return false;
    }

    fixture.source == observed.span()
}
