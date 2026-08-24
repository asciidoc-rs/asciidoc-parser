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
    // to descend — parse no blocks and leave `source` unconsumed — so a crafted
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
        // condition can consult the running document-attribute state — notably
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

        let after = match maw.item {
            BlockParseOutcome::Parsed(mi) => {
                let after = mi.after;
                blocks.push(mi.item);
                after
            }

            BlockParseOutcome::Dropped(after) => {
                // A dropped block (`attribute-missing=drop-line`) contributes
                // no block, but parsing must still advance past its source.
                after
            }

            BlockParseOutcome::NoMatch => {
                // `source` is non-blank by the loop guard and the
                // `discard_empty_lines` calls, yet no block could be made of
                // it. That happens when a line is not blank by
                // `Span::take_empty_line`'s reckoning (space and tab only) but
                // holds nothing a block can be built from — a lone vertical tab
                // (U+000B), form feed (U+000C), or carriage return, each of
                // which the block parsers discard as trailing whitespace
                // (issue #1234). Consume the line and carry on, exactly as if
                // it had been blank: leaving it unconsumed would spin this loop
                // forever, and breaking out of the loop would silently truncate
                // whatever follows it.
                source.take_normalized_line().after
            }
        };

        // Progress guarantee. Every outcome above is expected to advance past
        // at least one line of `source`, but the loop is bounded on the source
        // being non-empty, not on it having moved — so an outcome that reports
        // no progress (a future parser bug, or a case not yet foreseen) would
        // spin here forever rather than fail visibly. Stop instead: truncating
        // the parse loses content, but it terminates, and an infinite loop in
        // a library parsing untrusted input is the worse failure by far.
        if after.byte_offset() <= source.byte_offset() {
            break;
        }

        source = after.discard_empty_lines();
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
