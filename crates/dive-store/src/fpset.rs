//! The fingerprint set (ADR 0015 §5).

use crate::error::StoreError;
use crate::set::{
    Candidates, ResolveOptions, Resolved, Shard, StateSet, StoreMode, resolve_shards,
};

/// The initial number of slots per shard.
const MIN_SLOT_BITS: u32 = 4;

/// One shard: an open-addressing table of `u64` fingerprints with linear probing.
///
/// A slot holding 0 is empty; the fingerprint 0 itself is tracked by a flag, so no
/// fingerprint is remapped and no artificial collision is introduced. The slot of a
/// fingerprint is given by the bits just below the shard bits, so the slot order is
/// the fingerprint order: candidates sorted by fingerprint probe the table almost
/// sequentially.
#[derive(Clone, Debug)]
struct FpShard {
    slots: Vec<u64>,
    len: usize,
    zero: bool,
    /// The set's shard bits.
    shard_bits: u32,
    /// `slots.len() == 1 << slot_bits`.
    slot_bits: u32,
}

impl FpShard {
    fn new(shard_bits: u32, slot_bits: u32) -> Self {
        Self {
            slots: vec![0; 1 << slot_bits],
            len: 0,
            zero: false,
            shard_bits,
            slot_bits,
        }
    }

    fn home(&self, fp: u64) -> usize {
        let below = if self.shard_bits == 0 {
            fp
        } else {
            fp << self.shard_bits
        };
        usize::try_from(below >> (64 - self.slot_bits)).unwrap_or(0)
    }

    fn contains(&self, fp: u64) -> bool {
        if fp == 0 {
            return self.zero;
        }
        let mask = self.slots.len() - 1;
        let mut i = self.home(fp);
        loop {
            match self.slots[i] {
                0 => return false,
                s if s == fp => return true,
                _ => i = (i + 1) & mask,
            }
        }
    }

    fn insert(&mut self, fp: u64) -> bool {
        if fp == 0 {
            let new = !self.zero;
            self.zero = true;
            self.len += usize::from(new);
            return new;
        }
        // Keep the load factor at most 3/4.
        if (self.len + 1) * 4 > self.slots.len() * 3 {
            self.grow();
        }
        let mask = self.slots.len() - 1;
        let mut i = self.home(fp);
        loop {
            match self.slots[i] {
                0 => {
                    self.slots[i] = fp;
                    self.len += 1;
                    return true;
                }
                s if s == fp => return false,
                _ => i = (i + 1) & mask,
            }
        }
    }

    fn grow(&mut self) {
        let old = std::mem::take(&mut self.slots);
        self.slot_bits = (self.slot_bits + 1).min(64 - self.shard_bits);
        self.slots = vec![0; 1 << self.slot_bits];
        let mask = self.slots.len() - 1;
        for fp in old.into_iter().filter(|&fp| fp != 0) {
            let mut i = self.home(fp);
            while self.slots[i] != 0 {
                i = (i + 1) & mask;
            }
            self.slots[i] = fp;
        }
    }

    fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.zero
            .then_some(0)
            .into_iter()
            .chain(self.slots.iter().copied().filter(|&fp| fp != 0))
    }
}

impl Shard for FpShard {
    type Key = u64;
    fn insert_hashed(&mut self, _hash: u64, key: &u64) -> bool {
        self.insert(*key)
    }
}

/// A set of 64-bit state fingerprints in `2^k` shards selected by a fingerprint's top
/// `k` bits.
///
/// Memory is 8 bytes per slot at a load factor between 3/8 and 3/4: 10.7 to 21.3
/// bytes per state, plus the table being built while a shard grows.
///
/// ```
/// use dive_store::{FpSet, ResolveOptions, StateSet};
///
/// let mut set = FpSet::new(2);
/// assert!(set.insert(42));
///
/// // Two workers found candidates in a level; positions are their place in the
/// // reference order.
/// let mut w0 = set.candidates();
/// let mut w1 = set.candidates();
/// w0.push(&set, 7, 3u64, "seven at 3");
/// w1.push(&set, 7, 1u64, "seven at 1");
/// w1.push(&set, 42, 2u64, "already seen");
/// let won = set.resolve(vec![w0, w1], &ResolveOptions::threads(2))?;
/// let won = won.into_position_order();
/// assert_eq!(won.len(), 1);
/// assert_eq!((won[0].key, won[0].pos, won[0].payload), (7, 1, "seven at 1"));
/// assert_eq!(set.len(), 2);
/// # Ok::<(), dive_store::StoreError>(())
/// ```
#[derive(Clone, Debug)]
pub struct FpSet {
    shards: Vec<FpShard>,
    shard_bits: u32,
}

impl FpSet {
    /// An empty set with `2^shard_bits` shards (at most 2^16). See
    /// [`crate::shard_bits_for`].
    #[must_use]
    pub fn new(shard_bits: u32) -> Self {
        let shard_bits = shard_bits.min(16);
        Self {
            shards: (0..1usize << shard_bits)
                .map(|_| FpShard::new(shard_bits, MIN_SLOT_BITS))
                .collect(),
            shard_bits,
        }
    }

    /// All fingerprints, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.shards.iter().flat_map(FpShard::iter)
    }

    /// The smallest gap between two fingerprints that agree in their top `fp_bits`
    /// bits, as TLC's `checkFPs` computes it over its default `MultiFPSet` (which has
    /// `2^fp_bits` sub-sets, split by those bits; TLC's default is 1). This is the
    /// input to the "based on the actual fingerprints" estimate
    /// ([`crate::collision::actual_probability`]), meaningful when these are TLC's
    /// fingerprints. It is `i64::MAX` (Java's `Long.MAX_VALUE`) when no two
    /// fingerprints share a sub-set.
    #[must_use]
    pub fn tlc_min_gap(&self, fp_bits: u32) -> u64 {
        // Shards hold contiguous fingerprint ranges in shard order, so sorting each
        // shard and carrying the last fingerprint across gives the global order.
        let mut prev: Option<u64> = None;
        let mut gap = crate::collision::NO_GAP;
        for shard in &self.shards {
            let mut fps: Vec<u64> = shard.iter().collect();
            fps.sort_unstable();
            for fp in fps {
                if let Some(p) = prev {
                    gap = crate::collision::closer(gap, p, fp, fp_bits);
                }
                prev = Some(fp);
            }
        }
        gap
    }
}

impl StateSet for FpSet {
    type Key = u64;

    fn mode(&self) -> StoreMode {
        StoreMode::Fingerprint
    }

    fn hash(&self, key: &u64) -> u64 {
        *key
    }

    fn shard_count(&self) -> usize {
        self.shards.len()
    }

    fn contains(&self, key: &u64) -> bool {
        self.shards[crate::set::shard_of(*key, self.shard_bits)].contains(*key)
    }

    fn insert(&mut self, key: u64) -> bool {
        let i = crate::set::shard_of(key, self.shard_bits);
        self.shards[i].insert(key)
    }

    fn len(&self) -> u64 {
        self.shards.iter().map(|s| s.len as u64).sum()
    }

    fn memory_bytes(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.slots.capacity() * 8)
            .sum::<usize>()
            + self.shards.capacity() * std::mem::size_of::<FpShard>()
    }

    fn resolve<P, T>(
        &mut self,
        batches: Vec<Candidates<u64, P, T>>,
        opts: &ResolveOptions,
    ) -> Result<Resolved<u64, P, T>, StoreError>
    where
        P: Ord + Copy + Send,
        T: Send,
    {
        resolve_shards(&mut self.shards, batches, opts)
    }
}
