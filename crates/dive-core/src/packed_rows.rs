//! Lossless in-process blocks of frontier handles sharing the same live Store.
use crate::{Error, Handle};

/// Failure to store or read a resident frontier block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockError {
    /// The requested storage could not be reserved.
    Allocation,
    /// The input shape, requested range or internal column layout is invalid.
    Layout,
}

impl BlockError {
    /// Reports a failure while admitting or reading rows during ordinary search.
    #[must_use]
    pub fn exploration_error(self) -> Error {
        Error::internal(match self {
            BlockError::Allocation => "resident frontier block allocation",
            BlockError::Layout => "invalid resident frontier block layout",
        })
    }
}

/// Number of contiguous breadth-first rows in a sealed block.
pub const ROWS: usize = 256;

#[derive(Debug)]
struct Column {
    base: u64,
    offset: usize,
    bytes: usize,
}

#[derive(Debug)]
enum Data {
    Raw(Box<[Handle]>),
    Packed(Vec<Column>, Vec<u8>),
}

/// An immutable rectangular block of handles from one Store. The encoding is
/// private. The original handles may only be used with their original owner.
#[derive(Debug)]
pub struct Block {
    width: usize,
    data: Data,
}

impl Block {
    /// Packs exactly [`ROWS`] complete rows. An incompressible block keeps
    /// its original row-major values.
    ///
    /// # Errors
    /// Zero width, incorrect length, or insufficient workspace.
    pub fn encode(values: &[Handle], width: usize) -> Result<Self, BlockError> {
        if width == 0 || width.checked_mul(ROWS) != Some(values.len()) {
            return Err(BlockError::Layout);
        }
        let alloc = |_: std::collections::TryReserveError| BlockError::Allocation;
        let mut columns = Vec::new();
        columns.try_reserve_exact(width).map_err(alloc)?;
        let mut size = 0;
        for (column, &first) in values[..width].iter().enumerate() {
            let change = values[column..]
                .iter()
                .step_by(width)
                .fold(0, |mask, h| mask | (first.bits() ^ h.bits()));
            let bytes = match 64 - change.leading_zeros() {
                0 => 0,
                1..=8 => 1,
                9..=16 => 2,
                17..=32 => 4,
                _ => 8,
            };
            columns.push(Column {
                base: first.bits(),
                offset: size,
                bytes,
            });
            size += ROWS * bytes;
        }
        let data = if size_of_val(values) - size <= width * size_of::<Column>() {
            let mut raw = Vec::new();
            raw.try_reserve_exact(values.len()).map_err(alloc)?;
            raw.extend_from_slice(values);
            Data::Raw(raw.into_boxed_slice())
        } else {
            let mut packed = Vec::new();
            packed.try_reserve_exact(size).map_err(alloc)?;
            for (i, column) in columns.iter().enumerate().filter(|(_, c)| c.bytes != 0) {
                for handle in values[i..].iter().step_by(width) {
                    let low = (handle.bits() ^ column.base).to_le_bytes();
                    packed.extend_from_slice(&low[..column.bytes]);
                }
            }
            Data::Packed(columns, packed)
        };
        Ok(Self { width, data })
    }

    /// Compare a single row without allocating, using the caller's value equality.
    ///
    /// # Errors
    /// The row index is outside the block or the encoded layout is invalid.
    pub fn row_matches(
        &self,
        index: usize,
        values: &[Handle],
        equal: impl Fn(Handle, Handle) -> bool,
    ) -> Result<bool, BlockError> {
        if index >= ROWS {
            return Err(BlockError::Layout);
        }
        if values.len() != self.width {
            return Ok(false);
        }
        match &self.data {
            Data::Raw(rows) => Ok(rows[index * self.width..(index + 1) * self.width]
                .iter()
                .zip(values)
                .all(|(&a, &b)| equal(a, b))),
            Data::Packed(columns, data) => {
                for (column, &value) in columns.iter().zip(values) {
                    let start = column.offset + index * column.bytes;
                    let encoded = &data[start..start + column.bytes];
                    let delta = match encoded {
                        [] => 0,
                        [byte] => u64::from(*byte),
                        [a, b] => u64::from(u16::from_le_bytes([*a, *b])),
                        [a, b, c, d] => u64::from(u32::from_le_bytes([*a, *b, *c, *d])),
                        eight => {
                            u64::from_le_bytes(eight.try_into().map_err(|_| BlockError::Layout)?)
                        }
                    };
                    if !equal(Handle(column.base ^ delta), value) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    /// Appends the requested subrange in original row and variable order.
    ///
    /// # Errors
    /// Invalid range or insufficient workspace.
    pub fn append(
        &self,
        start: usize,
        end: usize,
        output: &mut Vec<Handle>,
    ) -> Result<(), BlockError> {
        if start > end || end > ROWS {
            return Err(BlockError::Layout);
        }
        let count = (end - start) * self.width;
        if count == 0 {
            return Ok(());
        }
        output
            .try_reserve(count)
            .map_err(|_| BlockError::Allocation)?;
        let offset = output.len();
        match &self.data {
            Data::Raw(rows) => {
                output.extend_from_slice(&rows[start * self.width..end * self.width]);
            }
            Data::Packed(columns, data) => {
                output.resize(offset + count, Handle::EMPTY);
                for (i, c) in columns.iter().enumerate() {
                    let target = output[offset + i..].iter_mut().step_by(self.width);
                    let src = &data[c.offset + start * c.bytes..c.offset + end * c.bytes];
                    match c.bytes {
                        0 => target.for_each(|v| *v = Handle(c.base)),
                        1 => target
                            .zip(src)
                            .for_each(|(v, &x)| *v = Handle(c.base ^ u64::from(x))),
                        2 => target.zip(src.as_chunks::<2>().0).for_each(|(v, &x)| {
                            *v = Handle(c.base ^ u64::from(u16::from_le_bytes(x)));
                        }),
                        4 => target.zip(src.as_chunks::<4>().0).for_each(|(v, &x)| {
                            *v = Handle(c.base ^ u64::from(u32::from_le_bytes(x)));
                        }),
                        8 => target
                            .zip(src.as_chunks::<8>().0)
                            .for_each(|(v, &x)| *v = Handle(c.base ^ u64::from_le_bytes(x))),
                        _ => return Err(BlockError::Layout),
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_and_raw_blocks_restore_every_bit_in_subrange_order() -> Result<(), BlockError> {
        for raw in [false, true] {
            let base = 0x9abc_3210_7654_fedc;
            let masks = if raw {
                [1 << 63; 5]
            } else {
                [0, 1, 1 << 12, 1 << 30, 1 << 63]
            };
            let values: Vec<_> = (0..ROWS)
                .flat_map(|row| {
                    masks.map(move |mask| Handle(base ^ if row % 2 == 1 { mask } else { 0 }))
                })
                .collect();
            let block = Block::encode(&values, masks.len())?;
            assert_eq!(matches!(block.data, Data::Raw(_)), raw);
            let mut output = vec![Handle::EMPTY];
            for (start, end) in [(0, 1), (1, 60), (60, 255), (255, 256), (256, 256)] {
                block.append(start, end, &mut output)?;
            }
            assert_eq!(output[1..], values);
            for (index, row) in values.chunks_exact(masks.len()).enumerate() {
                assert!(block.row_matches(index, row, |a, b| a == b)?);
                assert!(!block.row_matches(index, &row[1..], |a, b| a == b)?);
                assert!(!block.row_matches(index, row, |_, _| false)?);
            }
            assert_eq!(
                block.row_matches(ROWS, &[], |a, b| a == b),
                Err(BlockError::Layout)
            );
            assert!(block.append(5, 4, &mut output).is_err());
            assert!(block.append(0, ROWS + 1, &mut output).is_err());
        }
        assert!(Block::encode(&[], 0).is_err());
        assert!(Block::encode(&[Handle::EMPTY], usize::MAX).is_err());
        Ok(())
    }
}
