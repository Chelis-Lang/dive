//! Indexed, lossless parent deltas for the unbounded resident predecessor log.
use crate::trace::TraceStep;
use std::collections::TryReserveError;

const BLOCK: u64 = 64;
const ROOT: u64 = u64::MAX;

#[derive(Debug, Default)]
pub(super) struct Packed {
    bytes: Vec<u8>,
    // Byte position and preceding parent for each independently readable block.
    blocks: Vec<(usize, u64)>,
    previous: u64,
}

fn write_varint(bytes: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        bytes.push(n.to_le_bytes()[0] | 0x80);
        n >>= 7;
    }
    bytes.push(n.to_le_bytes()[0]);
}

fn read_varint(input: &mut std::slice::Iter<'_, u8>) -> Option<u64> {
    let mut n = 0;
    for shift in (0..64).step_by(7) {
        let byte = *input.next()?;
        n |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Some(n);
        }
    }
    None
}

impl Packed {
    pub(super) fn append(
        &mut self,
        id: u64,
        parent: Option<u64>,
        ordinal: u32,
    ) -> Result<(), TryReserveError> {
        let parent = parent.unwrap_or(ROOT);
        // Wrapping supports initial states and arbitrary legal parent order,
        // including restored logs. Wide or decreasing parents use the escape.
        let delta = parent.wrapping_sub(self.previous);
        let short = delta < 15 && ordinal < 16;
        self.bytes.try_reserve(if short { 1 } else { 16 })?;
        if id.is_multiple_of(BLOCK) {
            self.blocks.try_reserve(1)?;
            self.blocks.push((self.bytes.len(), self.previous));
        }
        if short {
            self.bytes
                .push(delta.to_le_bytes()[0] << 4 | ordinal.to_le_bytes()[0]);
        } else {
            self.bytes.push(255);
            write_varint(&mut self.bytes, delta);
            write_varint(&mut self.bytes, u64::from(ordinal));
        }
        self.previous = parent;
        Ok(())
    }

    pub(super) fn get(&self, id: u64) -> Option<TraceStep> {
        let &(offset, mut parent) = self.blocks.get(usize::try_from(id / BLOCK).ok()?)?;
        let mut input = self.bytes.get(offset..)?.iter();
        for index in 0..=id % BLOCK {
            let byte = *input.next()?;
            let (delta, ordinal) = if byte == 255 {
                (
                    read_varint(&mut input)?,
                    u32::try_from(read_varint(&mut input)?).ok()?,
                )
            } else {
                (u64::from(byte >> 4), u32::from(byte & 15))
            };
            parent = parent.wrapping_add(delta);
            if index == id % BLOCK {
                return Some(TraceStep {
                    id,
                    parent: (parent != ROOT).then_some(parent),
                    ordinal,
                });
            }
        }
        None
    }

    pub(super) fn memory_bytes(&self) -> usize {
        self.bytes.capacity() + self.blocks.capacity() * size_of::<(usize, u64)>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_across_blocks_and_escapes() -> Result<(), TryReserveError> {
        let mut packed = Packed::default();
        for id in 0..512 {
            let parent = match id % 6 {
                0 => None,
                1 => Some(0),
                _ => Some(id - 1),
            };
            let ordinal = if id % 2 == 0 { u32::MAX } else { 15 };
            packed.append(id, parent, ordinal)?;
            assert_eq!(
                packed.get(id),
                Some(TraceStep {
                    id,
                    parent,
                    ordinal
                })
            );
        }
        Ok(())
    }
}
