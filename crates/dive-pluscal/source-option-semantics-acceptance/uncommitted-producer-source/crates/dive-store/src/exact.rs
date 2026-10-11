//! The exact store (`--store exact`; ADR 0015 §5): full canonical state encodings.

use dive_value::codec::write_varint;

use crate::error::StoreError;
use crate::set::{
    Candidates, ResolveOptions, Resolved, Shard, StateSet, StoreMode, resolve_shards,
};

const MIN_SLOT_BITS: u32 = 4;

/// A deterministic 64-bit hash of a key's bytes, used only to place keys in shards
/// and slots. Equality is always decided by comparing the bytes, so the quality of
/// this hash affects speed, never results.
#[must_use]
pub fn key_hash(bytes: &[u8]) -> u64 {
    const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    let mut h = 0x243f_6a88_85a3_08d3 ^ (bytes.len() as u64);
    let (words, rest) = bytes.as_chunks::<8>();
    for w in words {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(*w)).wrapping_mul(K);
    }
    if !rest.is_empty() {
        let mut w = [0u8; 8];
        w[..rest.len()].copy_from_slice(rest);
        h = (h.rotate_left(5) ^ u64::from_le_bytes(w)).wrapping_mul(K);
    }
    // The `splitmix64` finalizer spreads every input bit over the top bits, which
    // choose the shard and the slot.
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

/// A slot: the key's hash and `1 +` its offset in the shard's arena (0: empty).
#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    hash: u64,
    at: u64,
}

/// One shard: an open-addressing table of slots over an append-only arena of keys
/// (each a varint length and the bytes).
#[derive(Clone, Debug)]
struct ExactShard {
    slots: Vec<Slot>,
    arena: Vec<u8>,
    len: usize,
    shard_bits: u32,
    slot_bits: u32,
}

impl ExactShard {
    fn new(shard_bits: u32) -> Self {
        Self {
            slots: vec![Slot::default(); 1 << MIN_SLOT_BITS],
            arena: Vec::new(),
            len: 0,
            shard_bits,
            slot_bits: MIN_SLOT_BITS,
        }
    }

    fn home(&self, hash: u64) -> usize {
        let below = if self.shard_bits == 0 {
            hash
        } else {
            hash << self.shard_bits
        };
        usize::try_from(below >> (64 - self.slot_bits)).unwrap_or(0)
    }

    /// The key stored at arena offset `at - 1`.
    fn key_at(&self, at: u64) -> &[u8] {
        let start = usize::try_from(at - 1).unwrap_or(usize::MAX);
        let mut input = self.arena.get(start..).unwrap_or(&[]);
        let n = dive_value::codec::read_varint(&mut input)
            .ok()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        input.get(..n).unwrap_or(&[])
    }

    /// The slot holding `key`, or the empty slot where it would go.
    fn find(&self, hash: u64, key: &[u8]) -> (usize, bool) {
        let mask = self.slots.len() - 1;
        let mut i = self.home(hash);
        loop {
            let s = self.slots[i];
            if s.at == 0 {
                return (i, false);
            }
            if s.hash == hash && self.key_at(s.at) == key {
                return (i, true);
            }
            i = (i + 1) & mask;
        }
    }

    fn insert(&mut self, hash: u64, key: &[u8]) -> bool {
        if (self.len + 1) * 4 > self.slots.len() * 3 {
            self.grow();
        }
        let (i, found) = self.find(hash, key);
        if found {
            return false;
        }
        let at = self.arena.len() as u64 + 1;
        write_varint(&mut self.arena, key.len() as u64);
        self.arena.extend_from_slice(key);
        self.slots[i] = Slot { hash, at };
        self.len += 1;
        true
    }

    fn grow(&mut self) {
        let old = std::mem::take(&mut self.slots);
        self.slot_bits = (self.slot_bits + 1).min(64 - self.shard_bits);
        self.slots = vec![Slot::default(); 1 << self.slot_bits];
        let mask = self.slots.len() - 1;
        for s in old.into_iter().filter(|s| s.at != 0) {
            let mut i = self.home(s.hash);
            while self.slots[i].at != 0 {
                i = (i + 1) & mask;
            }
            self.slots[i] = s;
        }
    }
}

impl Shard for ExactShard {
    type Key = Box<[u8]>;
    fn insert_hashed(&mut self, hash: u64, key: &Box<[u8]>) -> bool {
        self.insert(hash, key)
    }
}

/// A set of full states, keyed by their canonical encoding
/// ([`crate::state::state_key`]): two states are the same exactly when their keys
/// are equal, so a search over this set claims exhaustiveness unconditionally.
///
/// Memory is the key bytes (plus a one-byte length for keys under 128 bytes) and a
/// 16-byte slot at a load factor between 3/8 and 3/4.
///
/// The set has the same interface and the same deterministic resolve as
/// [`crate::FpSet`], so a search gives identical results in both modes unless
/// fingerprints collide.
///
/// ```
/// use dive_store::{ExactSet, StateSet, state::state_key};
/// use dive_value::{Ctx, Int, Value};
///
/// let ctx = Ctx::tlc();
/// let key = |vars: &[Value]| -> Result<Box<[u8]>, dive_store::StoreError> {
///     let mut k = Vec::new();
///     state_key(vars, ctx, &mut k)?;
///     Ok(k.into())
/// };
/// let mut set = ExactSet::new(0);
/// assert!(set.insert(key(&[Value::interval(Int::from(1), Int::from(2), ctx)])?));
/// // `{1, 2}` is the same state as `1..2`.
/// let same = Value::set(vec![Value::Int(2), Value::Int(1)], ctx)?;
/// assert!(set.contains(&key(&[same])?));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct ExactSet {
    shards: Vec<ExactShard>,
    shard_bits: u32,
}

impl ExactSet {
    /// An empty set with `2^shard_bits` shards (at most 2^16).
    #[must_use]
    pub fn new(shard_bits: u32) -> Self {
        let shard_bits = shard_bits.min(16);
        Self {
            shards: (0..1usize << shard_bits)
                .map(|_| ExactShard::new(shard_bits))
                .collect(),
            shard_bits,
        }
    }

    /// The total size of the stored keys, in bytes.
    #[must_use]
    pub fn key_bytes(&self) -> usize {
        self.shards.iter().map(|s| s.arena.len()).sum()
    }

    /// All stored keys, shard by shard in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.shards.iter().flat_map(|s| {
            let mut input = s.arena.as_slice();
            std::iter::from_fn(move || {
                let n = dive_value::codec::read_varint(&mut input).ok()?;
                let n = usize::try_from(n).ok()?;
                let (k, rest) = input.split_at_checked(n)?;
                input = rest;
                Some(k)
            })
        })
    }

    fn shard(&self, hash: u64) -> &ExactShard {
        &self.shards[crate::set::shard_of(hash, self.shard_bits)]
    }
}

impl StateSet for ExactSet {
    type Key = Box<[u8]>;

    fn mode(&self) -> StoreMode {
        StoreMode::Exact
    }

    fn hash(&self, key: &Box<[u8]>) -> u64 {
        key_hash(key)
    }

    fn shard_count(&self) -> usize {
        self.shards.len()
    }

    fn contains(&self, key: &Box<[u8]>) -> bool {
        let h = key_hash(key);
        self.shard(h).find(h, key).1
    }

    fn insert(&mut self, key: Box<[u8]>) -> bool {
        let h = key_hash(&key);
        let i = crate::set::shard_of(h, self.shard_bits);
        self.shards[i].insert(h, &key)
    }

    fn len(&self) -> u64 {
        self.shards.iter().map(|s| s.len as u64).sum()
    }

    fn memory_bytes(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.slots.capacity() * std::mem::size_of::<Slot>() + s.arena.capacity())
            .sum::<usize>()
            + self.shards.capacity() * std::mem::size_of::<ExactShard>()
    }

    fn resolve<P, T>(
        &mut self,
        batches: Vec<Candidates<Box<[u8]>, P, T>>,
        opts: &ResolveOptions,
    ) -> Result<Resolved<Box<[u8]>, P, T>, StoreError>
    where
        P: Ord + Copy + Send,
        T: Send,
    {
        resolve_shards(&mut self.shards, batches, opts)
    }
}
