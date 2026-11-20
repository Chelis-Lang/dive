//! Resident BFS rows. Complete blocks precede the unsealed row-major tail.
use std::ops::Range;

use dive_core::packed_rows::{Block, ROWS};
use dive_core::{Error, Handle};

pub(super) fn len(blocks: Option<&[Block]>, tail: &[Handle], width: usize) -> usize {
    blocks.map_or(0, |sealed| sealed.len() * ROWS) + tail.len().checked_div(width).unwrap_or(0)
}

pub(super) fn read(
    blocks: &[Block],
    tail: &[Handle],
    width: usize,
    rows: Range<usize>,
    output: &mut Vec<Handle>,
) -> Result<(), Error> {
    output.clear();
    let sealed = blocks.len() * ROWS;
    let mut pos = rows.start;
    while pos < rows.end.min(sealed) {
        let first = pos % ROWS;
        let count = (ROWS - first).min(rows.end - pos);
        blocks[pos / ROWS].append(first, first + count, output)?;
        pos += count;
    }
    if pos < rows.end {
        let slice = tail
            .get((pos - sealed) * width..(rows.end - sealed) * width)
            .ok_or_else(|| Error::internal("invalid resident frontier tail"))?;
        output
            .try_reserve(slice.len())
            .map_err(|_| Error::internal("resident frontier read allocation"))?;
        output.extend_from_slice(slice);
    }
    Ok(())
}
