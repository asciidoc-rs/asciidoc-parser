use crate::{
    Parser, Span,
    blocks::{Block, block::BlockParseOutcome},
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

        let mut maw = Block::parse_with_outcome(source, parser);

        if !maw.warnings.is_empty() {
            warnings.append(&mut maw.warnings);
        }

        if let BlockParseOutcome::Parsed(mi) = maw.item {
            source = mi.after.discard_empty_lines();
            blocks.push(mi.item);
        } else if let BlockParseOutcome::Dropped(after) = maw.item {
            // A dropped block (`attribute-missing=drop-line`) contributes no
            // block, but parsing must still advance past its source.
            source = after.discard_empty_lines();
        }

        // `BlockParseOutcome::NoMatch` is intentionally not handled: it only
        // arises for empty/blank input, which never reaches here because the
        // loop guard and the `discard_empty_lines` calls above keep `source`
        // non-blank. (Matching the behavior before drop-line support, which
        // likewise only advanced on a successful parse.)
    }

    MatchAndWarnings {
        item: MatchedItem {
            item: blocks,
            after: source,
        },
        warnings,
    }
}
