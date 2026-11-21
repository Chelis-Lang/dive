//! Append-only rows of retained mathematical identities in one live store.

use dive_core::packed_rows::{Block, BlockError, ROWS};
use dive_core::{Error, Handle, Heap};

#[derive(Debug, Default)]
struct Plain {
    values: Vec<Handle>,
    ends: Vec<usize>,
}

impl Plain {
    fn row(&self, index: usize) -> Result<&[Handle], Error> {
        let end = *self.ends.get(index).ok_or_else(invalid_index)?;
        let start = index.checked_sub(1).map_or(0, |i| self.ends[i]);
        self.values.get(start..end).ok_or_else(invalid_index)
    }
}

#[derive(Debug)]
enum Sealed {
    Packed(Block),
    Plain(Plain),
}

#[derive(Debug, Default)]
pub(super) struct IdentityRows {
    sealed: Vec<Sealed>,
    tail: Plain,
}

fn invalid_index() -> Error {
    Error::internal("invalid exact state row index")
}

fn block_error(error: BlockError) -> Error {
    match error {
        BlockError::Allocation => Error::unsupported("exact state row allocation"),
        BlockError::Layout => invalid_index(),
    }
}

fn allocation(_: std::collections::TryReserveError) -> Error {
    Error::unsupported("exact state row allocation")
}

impl IdentityRows {
    /// Reserve the whole new row before publishing the hash index entry.
    pub(super) fn reserve(&mut self, width: usize) -> Result<usize, Error> {
        if self.tail.ends.len() == ROWS {
            self.sealed.try_reserve(1).map_err(allocation)?;
            let columns = self.tail.ends[0];
            let rectangular = (1..=256).contains(&columns)
                && self
                    .tail
                    .ends
                    .iter()
                    .enumerate()
                    .all(|(i, &end)| end == (i + 1) * columns);
            let block = if rectangular {
                let packed = Block::encode(&self.tail.values, columns).map_err(block_error)?;
                self.tail.values.clear();
                self.tail.ends.clear();
                Sealed::Packed(packed)
            } else {
                Sealed::Plain(std::mem::take(&mut self.tail))
            };
            self.sealed.push(block);
        }
        self.tail.values.try_reserve(width).map_err(allocation)?;
        self.tail.ends.try_reserve(1).map_err(allocation)?;
        self.sealed
            .len()
            .checked_mul(ROWS)
            .and_then(|base| base.checked_add(self.tail.ends.len()))
            .ok_or_else(invalid_index)
    }

    pub(super) fn push_reserved(&mut self, row: &[Handle]) {
        self.tail.values.extend_from_slice(row);
        self.tail.ends.push(self.tail.values.len());
    }

    pub(super) fn matches(
        &self,
        position: usize,
        row: &[Handle],
        heap: &Heap,
    ) -> Result<bool, Error> {
        let offset = position % ROWS;
        let kept = match self.sealed.get(position / ROWS) {
            Some(Sealed::Packed(block)) => {
                return block
                    .row_matches(offset, row, |a, b| heap.eq(a, b))
                    .map_err(block_error);
            }
            Some(Sealed::Plain(block)) => block.row(offset)?,
            None if position / ROWS == self.sealed.len() => self.tail.row(offset)?,
            None => return Err(invalid_index()),
        };
        Ok(kept.len() == row.len() && kept.iter().zip(row).all(|(&a, &b)| heap.eq(a, b)))
    }

    /// Borrow an available plain row or decode one packed row into caller-owned storage.
    pub(super) fn read<'a>(
        &'a self,
        position: usize,
        scratch: &'a mut Vec<Handle>,
    ) -> Result<&'a [Handle], Error> {
        let offset = position % ROWS;
        match self.sealed.get(position / ROWS) {
            Some(Sealed::Packed(block)) => {
                scratch.clear();
                block
                    .append(offset, offset + 1, scratch)
                    .map_err(block_error)?;
                Ok(scratch)
            }
            Some(Sealed::Plain(block)) => block.row(offset),
            None if position / ROWS == self.sealed.len() => self.tail.row(offset),
            None => Err(invalid_index()),
        }
    }
}
