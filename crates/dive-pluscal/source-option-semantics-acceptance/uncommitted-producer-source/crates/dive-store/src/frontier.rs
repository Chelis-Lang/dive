//! Per-level frontiers of encoded states (ADR 0015 §2, §8).
//!
//! A level's frontier `F_L` holds its states, losslessly encoded
//! ([`crate::state::encode_state`]), in the reference order 𝒪. States are numbered
//! in discovery order, so a level's states have consecutive ids starting at
//! [`Frontier::first_id`] and no per-state id is stored.
//!
//! In the parallel engine each block of level `L` produces its winners, in position
//! order, as a [`StateBlock`]; appending the blocks in block order to a
//! [`FrontierBuilder`] gives `F_{L+1}` in 𝒪 order without copying.

use std::ops::Range;

use crate::error::StoreError;
use crate::trace::StateId;

/// The largest encoded state, and the size at which a block starts a new chunk.
const CHUNK_BYTES: usize = 1 << 30;

/// A sequence of encoded states in one buffer.
#[derive(Clone, Debug, Default)]
pub struct StateBlock {
    bytes: Vec<u8>,
    /// `ends[i]` is the end offset of state `i`.
    ends: Vec<u32>,
}

impl StateBlock {
    /// An empty block.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an encoded state.
    ///
    /// # Errors
    /// [`StoreError::StateTooLarge`] if the block would exceed 1 GiB.
    pub fn push(&mut self, state: &[u8]) -> Result<(), StoreError> {
        self.push_with(|buf| {
            buf.extend_from_slice(state);
            Ok(())
        })
    }

    /// Appends a state encoded in place by `encode`, which appends to the buffer it
    /// is given (for example [`crate::state::encode_state`]). If `encode` fails, the
    /// block is unchanged.
    ///
    /// # Errors
    /// The error of `encode`, or [`StoreError::StateTooLarge`].
    pub fn push_with(
        &mut self,
        encode: impl FnOnce(&mut Vec<u8>) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        let start = self.bytes.len();
        if let Err(e) = encode(&mut self.bytes) {
            self.bytes.truncate(start);
            return Err(e);
        }
        match u32::try_from(self.bytes.len()) {
            Ok(end) if self.bytes.len() <= CHUNK_BYTES => {
                self.ends.push(end);
                Ok(())
            }
            _ => {
                let n = self.bytes.len() - start;
                self.bytes.truncate(start);
                Err(StoreError::StateTooLarge(n))
            }
        }
    }

    /// The number of states.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    /// Whether the block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// The `i`-th encoded state.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&[u8]> {
        let end = usize::try_from(*self.ends.get(i)?).ok()?;
        let start = match i.checked_sub(1) {
            Some(p) => usize::try_from(*self.ends.get(p)?).ok()?,
            None => 0,
        };
        self.bytes.get(start..end)
    }

    /// The encoded states in order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        (0..self.len()).filter_map(|i| self.get(i))
    }

    /// Heap memory held, in bytes.
    #[must_use]
    pub fn memory_bytes(&self) -> usize {
        self.bytes.capacity() + self.ends.capacity() * 4
    }

    /// The encoded size of the states, in bytes.
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        self.bytes.len()
    }

    fn would_overflow(&self) -> bool {
        self.bytes.len() >= CHUNK_BYTES
    }
}

/// Builds the frontier of one level by appending states or whole blocks in order.
#[derive(Debug)]
pub struct FrontierBuilder {
    level: u32,
    first_id: StateId,
    chunks: Vec<StateBlock>,
}

impl FrontierBuilder {
    /// An empty frontier for level `level` whose first state has id `first_id` (the
    /// number of states discovered before it).
    #[must_use]
    pub fn new(level: u32, first_id: StateId) -> Self {
        Self {
            level,
            first_id,
            chunks: Vec::new(),
        }
    }

    fn tail(&mut self) -> &mut StateBlock {
        if self.chunks.last().is_none_or(StateBlock::would_overflow) {
            self.chunks.push(StateBlock::new());
        }
        let n = self.chunks.len() - 1;
        &mut self.chunks[n]
    }

    /// Appends an encoded state.
    ///
    /// # Errors
    /// [`StoreError::StateTooLarge`] for a state over 1 GiB.
    pub fn push(&mut self, state: &[u8]) -> Result<(), StoreError> {
        self.tail().push(state)
    }

    /// Appends a state encoded in place (see [`StateBlock::push_with`]).
    ///
    /// # Errors
    /// The error of `encode`, or [`StoreError::StateTooLarge`].
    pub fn push_with(
        &mut self,
        encode: impl FnOnce(&mut Vec<u8>) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        self.tail().push_with(encode)
    }

    /// Appends a whole block, after the states already added, without copying.
    pub fn append_block(&mut self, block: StateBlock) {
        if !block.is_empty() {
            self.chunks.push(block);
        }
    }

    /// The frontier.
    #[must_use]
    pub fn finish(self) -> Frontier {
        let chunks: Vec<StateBlock> = self.chunks.into_iter().filter(|c| !c.is_empty()).collect();
        let mut starts = Vec::with_capacity(chunks.len());
        let mut len = 0;
        for c in &chunks {
            starts.push(len);
            len += c.len();
        }
        Frontier {
            level: self.level,
            first_id: self.first_id,
            chunks,
            starts,
            len,
        }
    }
}

/// The encoded states of one level, in 𝒪 order.
///
/// ```
/// use dive_store::frontier::{FrontierBuilder, StateBlock};
///
/// let mut b = FrontierBuilder::new(2, 10);
/// b.push(b"s10")?;
/// let mut block = StateBlock::new();
/// block.push(b"s11")?;
/// block.push(b"s12")?;
/// b.append_block(block);
/// let f = b.finish();
/// assert_eq!(f.len(), 3);
/// assert_eq!(f.get(2), Some(&b"s12"[..]));
/// assert_eq!(f.id(2), Some(12));
/// # Ok::<(), dive_store::StoreError>(())
/// ```
#[derive(Debug)]
pub struct Frontier {
    level: u32,
    first_id: StateId,
    chunks: Vec<StateBlock>,
    /// `starts[c]` is the index of chunk `c`'s first state.
    starts: Vec<usize>,
    len: usize,
}

impl Frontier {
    /// An empty frontier.
    #[must_use]
    pub fn empty(level: u32, first_id: StateId) -> Self {
        FrontierBuilder::new(level, first_id).finish()
    }

    /// The level (1 for the initial states).
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }

    /// The id of the first state.
    #[must_use]
    pub const fn first_id(&self) -> StateId {
        self.first_id
    }

    /// The number of states.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the frontier is empty (the search is complete).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The `i`-th encoded state.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&[u8]> {
        let c = self.starts.partition_point(|&s| s <= i).checked_sub(1)?;
        self.chunks.get(c)?.get(i - self.starts.get(c)?)
    }

    /// The state id of the `i`-th state.
    #[must_use]
    pub fn id(&self, i: usize) -> Option<StateId> {
        (i < self.len).then(|| self.first_id + i as u64)
    }

    /// The encoded states in order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.chunks.iter().flat_map(StateBlock::iter)
    }

    /// Splits the frontier into blocks for `workers` workers: about 16 blocks per
    /// worker, each of 1 to 256 states (ADR 0015 §2). The blocks are consecutive and
    /// cover every state, so concatenating per-block results in block order keeps
    /// 𝒪 order.
    #[must_use]
    pub fn blocks(&self, workers: usize) -> Vec<Range<usize>> {
        let size = (self.len / (16 * workers.max(1))).clamp(1, 256);
        (0..self.len)
            .step_by(size)
            .map(|s| s..(s + size).min(self.len))
            .collect()
    }

    /// Whether the level is narrow for `workers` workers (fewer than two states per
    /// worker), so that the engine should split work by action instead of by state.
    #[must_use]
    pub const fn is_narrow(&self, workers: usize) -> bool {
        self.len < 2 * workers
    }

    /// Heap memory held, in bytes.
    #[must_use]
    pub fn memory_bytes(&self) -> usize {
        self.chunks
            .iter()
            .map(StateBlock::memory_bytes)
            .sum::<usize>()
            + self.starts.capacity() * std::mem::size_of::<usize>()
    }

    /// The encoded size of the states, in bytes.
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        self.chunks.iter().map(StateBlock::encoded_bytes).sum()
    }
}
