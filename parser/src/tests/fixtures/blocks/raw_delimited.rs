use std::fmt;

use crate::{
    HasSpan,
    blocks::{ContentModel, IsBlock},
    content::SubstitutionGroup,
    tests::fixtures::{Span, attributes::Attrlist, content::Content},
};

#[derive(Eq, PartialEq)]
pub(crate) struct RawDelimitedBlock {
    pub content: Content,
    pub content_model: ContentModel,
    pub context: &'static str,
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub caption: Option<&'static str>,
    pub number: Option<usize>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
    pub substitution_group: SubstitutionGroup,
}

impl fmt::Debug for RawDelimitedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawDelimitedBlock")
            .field("content", &self.content)
            .field("content_model", &self.content_model)
            .field("context", &self.context)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("caption", &self.caption)
            .field("number", &self.number)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .field("substitution_group", &self.substitution_group)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::RawDelimitedBlock<'src>> for RawDelimitedBlock {
    fn eq(&self, other: &crate::blocks::RawDelimitedBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<RawDelimitedBlock> for crate::blocks::RawDelimitedBlock<'_> {
    fn eq(&self, other: &RawDelimitedBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(
    fixture: &RawDelimitedBlock,
    observed: &crate::blocks::RawDelimitedBlock,
) -> bool {
    if &fixture.content != observed.content() {
        return false;
    }

    if fixture.content_model != observed.content_model() {
        return false;
    }

    if fixture.context != observed.raw_context().as_ref() {
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

    if fixture.number != observed.number() {
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

    fixture.source == observed.span() && fixture.substitution_group == observed.substitution_group()
}
