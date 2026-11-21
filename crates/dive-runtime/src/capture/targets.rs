//! Random-access storage for exact destination IDs in discovery order.

use std::collections::TryReserveError;
use std::sync::OnceLock;

const BLOCK: usize = 64;

/// The retained target column exceeded its physical index range or allocation capacity.
#[derive(Clone, Copy, Debug)]
pub struct TargetCapacityError;

impl std::fmt::Display for TargetCapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("target column exceeds address or allocation capacity")
    }
}

impl std::error::Error for TargetCapacityError {}

impl From<TryReserveError> for TargetCapacityError {
    fn from(_: TryReserveError) -> Self {
        Self
    }
}

#[derive(Debug)]
struct Block {
    start: usize,
    base: u32,
    width: u8,
    runs: bool,
}

/// An append-only column of exact destination IDs. Completed 64-edge blocks
/// store base-relative values and optionally a bitmap of run starts.
///
/// ```
/// use dive_runtime::capture::{CsrTargets, PackedTargets};
/// let mut targets = PackedTargets::default();
/// for value in [1, 1, 9] { targets.try_push(value)?; }
/// let view = CsrTargets::from(&targets);
/// assert_eq!(view.prefix(2).and_then(|p| p.get(1)), Some(1));
/// assert_eq!(view.get(2), Some(9));
/// # Ok::<(), dive_runtime::capture::TargetCapacityError>(())
/// ```
#[derive(Debug, Default)]
pub struct PackedTargets {
    leading: Vec<u32>,
    directory: Vec<Block>,
    words: Vec<u64>,
    tail: Vec<u32>,
    dense: OnceLock<Vec<u32>>,
}

impl PackedTargets {
    /// Number of stored physical transitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leading.len() + self.directory.len() * BLOCK + self.tail.len()
    }

    /// Whether the column has any physical transitions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Exact backing capacities for the dense prefix, directory, value words and tail.
    /// A dense slice explicitly materialized by a legacy reader is separate.
    #[must_use]
    pub fn capacities(&self) -> [usize; 4] {
        [
            self.leading.capacity() * size_of::<u32>(),
            self.directory.capacity() * size_of::<Block>(),
            self.words.capacity() * size_of::<u64>(),
            self.tail.capacity() * size_of::<u32>(),
        ]
    }

    /// Appends one occurrence, preserving all earlier IDs and their positions.
    ///
    /// # Errors
    /// The physical index would overflow or a backing allocation could not be reserved.
    pub fn try_push(&mut self, target: u32) -> Result<(), TargetCapacityError> {
        if self.len() == usize::MAX {
            return Err(TargetCapacityError);
        }
        if self.tail.len() == BLOCK {
            self.seal()?;
        }
        self.tail.try_reserve(1)?;
        self.dense.take();
        self.tail.push(target);
        Ok(())
    }

    fn seal(&mut self) -> Result<(), TryReserveError> {
        let mut minimum = self.tail[0];
        let mut maximum = minimum;
        let mut mask = 1u64;
        for (i, pair) in self.tail.windows(2).enumerate() {
            minimum = minimum.min(pair[1]);
            maximum = maximum.max(pair[1]);
            if pair[0] != pair[1] {
                mask |= 1 << (i + 1);
            }
        }
        #[allow(clippy::cast_possible_truncation, reason = "a u32 has at most 32 bits")]
        let mut width = (maximum - minimum).bit_width() as u8;
        let mut bits = usize::from(width);
        let run_words = 1 + (mask.count_ones() as usize * bits).div_ceil(64);
        let mut runs = run_words < bits;
        let mut count = if runs { run_words } else { bits };
        if count * size_of::<u64>() + size_of::<Block>() >= BLOCK * size_of::<u32>() {
            if self.directory.is_empty() {
                self.leading.try_reserve(BLOCK)?;
                self.leading.extend_from_slice(&self.tail);
                self.tail.clear();
                return Ok(());
            }
            width = 32;
            bits = 32;
            count = 32;
            runs = false;
        }
        let base = if width == 32 { 0 } else { minimum };
        self.directory.try_reserve(1)?;
        self.words.try_reserve(count)?;
        let start = self.words.len();
        self.directory.push(Block {
            start,
            base,
            width,
            runs,
        });
        self.words.resize(start + count, 0);
        if runs {
            self.words[start] = mask;
        }
        if bits != 0 {
            let data = &mut self.words[start + usize::from(runs)..];
            let mut offset = 0;
            for (pos, &id) in self.tail.iter().enumerate() {
                if !runs || mask & (1 << pos) != 0 {
                    let word = offset / 64;
                    let shift = offset % 64;
                    let delta = u64::from(id - base);
                    data[word] |= delta << shift;
                    if shift + bits > 64 {
                        data[word + 1] |= delta >> (64 - shift);
                    }
                    offset += bits;
                }
            }
        }
        self.tail.clear();
        Ok(())
    }

    fn at(&self, edge: usize) -> u32 {
        if edge < self.leading.len() {
            return self.leading[edge];
        }
        let edge = edge - self.leading.len();
        let block = edge / BLOCK;
        let pos = edge % BLOCK;
        let Some(b) = self.directory.get(block) else {
            return self.tail[pos];
        };
        if b.width == 0 {
            return b.base;
        }
        let width = usize::from(b.width);
        let rank = if b.runs {
            let through = u64::MAX >> (63 - pos);
            (self.words[b.start] & through).count_ones() as usize - 1
        } else {
            pos
        };
        let bit = rank * width;
        let word = b.start + usize::from(b.runs) + bit / 64;
        let shift = bit % 64;
        let mut value = self.words[word] >> shift;
        if shift + width > 64 {
            value |= self.words[word + 1] << (64 - shift);
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "masked to the stored width of at most 32 bits"
        )]
        let delta = (value & ((1u64 << width) - 1)) as u32;
        b.base + delta
    }
}

/// Borrowed exact target IDs, optionally ending inside a sealed block.
/// Dense callers retain their original slice and its lifetime.
#[derive(Clone, Copy, Debug)]
pub enum CsrTargets<'a> {
    /// One target per occurrence.
    Dense(&'a [u32]),
    /// An immutable checked prefix of an append-only packed column.
    Packed(PackedTargetPrefix<'a>),
}

/// A prefix cannot extend beyond its parent column.
#[derive(Clone, Copy, Debug)]
pub struct PackedTargetPrefix<'a> {
    data: &'a PackedTargets,
    len: usize,
}

impl<'a> From<&'a PackedTargets> for CsrTargets<'a> {
    fn from(data: &'a PackedTargets) -> Self {
        if data.leading.is_empty() && data.directory.is_empty() {
            Self::Dense(&data.tail)
        } else {
            Self::Packed(PackedTargetPrefix {
                data,
                len: data.len(),
            })
        }
    }
}

impl<'a, T: AsRef<[u32]> + ?Sized> From<&'a T> for CsrTargets<'a> {
    fn from(data: &'a T) -> Self {
        Self::Dense(data.as_ref())
    }
}

impl<'a, T: AsRef<[u32]> + ?Sized> From<&'a mut T> for CsrTargets<'a> {
    fn from(data: &'a mut T) -> Self {
        Self::Dense((*data).as_ref())
    }
}

impl<'a> CsrTargets<'a> {
    /// Physical occurrence count.
    #[must_use]
    pub const fn len(self) -> usize {
        match self {
            Self::Dense(v) => v.len(),
            Self::Packed(p) => p.len,
        }
    }

    /// Whether there are any occurrences.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// The first `edges` occurrences without expanding any blocks.
    #[must_use]
    pub fn prefix(self, edges: usize) -> Option<Self> {
        if edges > self.len() {
            return None;
        }
        Some(match self {
            Self::Dense(v) => Self::Dense(&v[..edges]),
            Self::Packed(p) => Self::Packed(PackedTargetPrefix { len: edges, ..p }),
        })
    }

    /// Exact destination, or `None` beyond this view.
    #[must_use]
    pub fn get(self, edge: usize) -> Option<u32> {
        match self {
            Self::Dense(v) => v.get(edge).copied(),
            Self::Packed(p) => (edge < p.len).then(|| p.data.at(edge)),
        }
    }

    pub(super) fn as_slice(self) -> &'a [u32] {
        match self {
            Self::Dense(v) => v,
            Self::Packed(p) => {
                let full = p
                    .data
                    .dense
                    .get_or_init(|| (0..p.data.len()).map(|i| p.data.at(i)).collect());
                &full[..p.len]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_target_blocks_round_trip_all_prefixes_and_random_reads()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut originals = Vec::new();
        let mut packed = PackedTargets::default();
        let mut seed = 0x983c_2013u32;
        for i in 0..1_113 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let id = match i / 64 {
                0 | 1 | 6 => seed,
                2 | 7 => u32::MAX,
                3 => u32::MAX - i % 13,
                4 => 88 + (i / 11) * 2_017,
                5 => seed & 0x1ffff,
                _ => i % 57,
            };
            originals.push(id);
            packed.try_push(id)?;
            let view = CsrTargets::from(&packed);
            assert_eq!(view.len(), originals.len());
            for edge in (0..originals.len()).rev() {
                assert_eq!(view.get(edge), Some(originals[edge]));
            }
            assert_eq!(view.get(originals.len()), None);
            let prefix_len = originals.len() * 3 / 4;
            let prefix = view.prefix(prefix_len).ok_or("missing checked prefix")?;
            assert_eq!(prefix.get(prefix_len), None);
            if i % 29 == 0 {
                assert_eq!(prefix.as_slice(), &originals[..prefix_len]);
                assert_eq!(view.as_slice(), &originals);
            }
            assert!(view.prefix(view.len() + 1).is_none());
        }
        assert_eq!(packed.leading.len(), 128);
        assert!(packed.directory.iter().any(|b| b.width == 0));
        assert!(packed.directory.iter().any(|b| b.width == 32 && !b.runs));
        assert!(packed.directory.iter().any(|b| b.runs));
        Ok(())
    }

    #[test]
    fn incompressible_columns_keep_dense_prefix_with_no_directory()
    -> Result<(), TargetCapacityError> {
        let mut packed = PackedTargets::default();
        for i in 0..16_384 {
            packed.try_push(if i % 2 == 0 { u32::MAX } else { 0 })?;
        }
        assert!(packed.directory.is_empty());
        assert_eq!(packed.words, [] as [u64; 0]);
        assert_eq!(packed.leading.len() + packed.tail.len(), 16_384);
        assert_eq!(CsrTargets::from(&packed).get(16_382), Some(u32::MAX));
        Ok(())
    }
}
