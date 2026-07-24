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
    F: FnMut(&Span<'src>, &Parser) -> bool,
{
    let mut blocks: Vec<Block<'src>> = vec![];
    let mut warnings: Vec<Warning<'src>> = vec![];

    source = source.discard_empty_lines();

    // Bound native recursion (issue #885). This scope is one level of block
    // nesting; if the running depth already exceeds `max-block-nesting`, refuse
    // to descend – parse no blocks and leave `source` unconsumed – so a crafted
    // document (strictly-increasing delimiters, deeply-nested sections, …)
    // cannot overflow the stack and abort the host. Non-empty over-nested
    // content is truncated with a warning; genuinely empty content is dropped
    // silently (nothing is lost).
    if parser.block_nesting_limit_reached() {
        if !source.data().is_empty() {
            parser.warn_block_nesting_exceeded(source.take_normalized_line().item, &mut warnings);
        }

        return MatchAndWarnings {
            item: MatchedItem {
                item: blocks,
                after: source,
            },
            warnings,
        };
    }

    parser.block_nesting_depth += 1;

    while !source.data().is_empty() {
        // The predicate is given the parser (as a shared borrow) so a stop
        // condition can consult the running document-attribute state – notably
        // `leveloffset`, which shifts the effective level a section boundary is
        // compared against. Every block preceding `source` (including any
        // `:leveloffset:` attribute entry) has already been applied to the
        // parser at this point, so the offset it reads is current.
        if f(&source, parser) {
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

    parser.block_nesting_depth -= 1;

    MatchAndWarnings {
        item: MatchedItem {
            item: blocks,
            after: source,
        },
        warnings,
    }
}
