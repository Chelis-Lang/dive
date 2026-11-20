//! Resident BFS rows. Complete blocks precede the unsealed row-major tail.
use std::ops::Range;

use dive_core::packed_rows::{Block, ROWS};
use dive_core::{Error, Handle};

/// Predecessor-log IDs for one level of newly admitted resident states.
#[derive(Default)]
pub(super) struct IdSpan {
    first: u64,
    count: u64,
}

impl IdSpan {
    pub(super) fn push(&mut self, id: u64) -> Result<(), Error> {
        if self.count == 0 {
            self.first = id;
        } else if self.first.checked_add(self.count) != Some(id) {
            return Err(Error::internal(
                "nonconsecutive resident frontier discovery ID",
            ));
        }
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::internal("resident frontier discovery count overflow"))?;
        Ok(())
    }

    pub(super) fn get(&self, index: usize) -> Result<u64, Error> {
        let index = index as u64;
        self.first
            .checked_add(index)
            .filter(|_| index < self.count)
            .ok_or_else(|| Error::internal("missing resident frontier discovery ID"))
    }
}

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
        blocks[pos / ROWS]
            .append(first, first + count, output)
            .map_err(dive_core::packed_rows::BlockError::exploration_error)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_span_rejects_gaps_and_wraparound_without_changing_previous_ids()
    -> Result<(), Error> {
        let mut ids = IdSpan::default();
        assert!(ids.get(0).is_err());
        let first = u64::from(u32::MAX);
        ids.push(first)?;
        assert!(ids.push(first).is_err());
        assert!(ids.push(first + 2).is_err());
        ids.push(first + 1)?;
        assert_eq!(ids.get(1)?, first + 1);
        assert!(ids.get(2).is_err());
        let mut last = IdSpan::default();
        last.push(u64::MAX)?;
        assert!(last.push(0).is_err());
        assert_eq!(last.get(0)?, u64::MAX);
        Ok(())
    }
}
