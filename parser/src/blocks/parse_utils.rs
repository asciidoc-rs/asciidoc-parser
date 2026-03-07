use crate::{
    Parser, Span,
    blocks::Block,
    span::MatchedItem,
    warnings::{MatchAndWarnings, Warning},
};

/// Parse blocks until end of input or a pre-determined stop condition is
/// reached.
pub(crate) fn parse_blocks_until<'src, F>(
    mut source: Span<'src>,
    mut f: F,
    parser: &mut Parser,
) -> MatchAndWarnings<'src, MatchedItem<'src, Vec<Block<'src>>>>
where
    F: FnMut(&Span<'src>) -> bool,
{
    let mut blocks: Vec<Block<'src>> = vec![];
    let mut warnings: Vec<Warning<'src>> = vec![];

    source = source.discard_empty_lines();

    while !source.data().is_empty() {
        if f(&source) {
            break;
        }

        let mut maw = Block::parse(source, parser);

        if !maw.warnings.is_empty() {
            warnings.append(&mut maw.warnings);
        }

        if let Some(mi) = maw.item {
            source = mi.after.discard_empty_lines();
            blocks.push(mi.item);
        }
    }

    MatchAndWarnings {
        item: MatchedItem {
            item: blocks,
            after: source,
        },
        warnings,
    }
}
