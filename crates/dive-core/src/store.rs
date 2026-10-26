//! The persistent, hash-consed, sharded store (`03-value-model.md` §3.1).
//!
//! - 256 shards, selected by the top eight bits of a value's hash.
//! - Node storage is an append-only chunked vector of `OnceLock` slots: a
//!   write happens once, under the shard's lock, and reads need no lock.
//! - The deduplication index (hash to node index) is consulted and updated
//!   only while interning, under the shard's `Mutex`.
//!
//! All of this is safe Rust. A handle the store did not issue is an internal
//! error: reads return a poison node and set [`Store::poisoned`], which the
//! engine reports as a bug, never as a verdict.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::handle::Handle;
use crate::node::{Kind, View};
use crate::strings::Strings;

const SHARDS: usize = 256;
/// The first chunk holds `2^BASE_BITS` nodes; chunk `k` holds `2^(BASE_BITS + k)`.
const BASE_BITS: u32 = 6;
/// Enough chunks for 2^48 nodes per shard.
const CHUNKS: usize = 43;

/// A node of the persistent store.
#[derive(Debug)]
pub(crate) struct PNode {
    pub(crate) kind: Kind,
    pub(crate) lo: i64,
    pub(crate) hash: u64,
    pub(crate) sum: u64,
    pub(crate) data: Box<[Handle]>,
}

impl PNode {
    pub(crate) fn view(&self) -> View<'_> {
        View {
            kind: self.kind,
            lo: self.lo,
            hash: self.hash,
            sum: self.sum,
            data: &self.data,
        }
    }
}

static POISON: OnceLock<PNode> = OnceLock::new();

fn poison() -> &'static PNode {
    POISON.get_or_init(|| PNode {
        kind: Kind::Set,
        lo: 0,
        hash: 0,
        sum: 0,
        data: Box::default(),
    })
}

#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        // The top bits select the shard, so they are constant within one;
        // rotate them out of the bits the table uses.
        self.0.rotate_left(32)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 << 8) | u64::from(b);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = v;
    }
}

type IdMap = HashMap<u64, u64, BuildHasherDefault<IdHasher>>;

#[derive(Default)]
struct Index {
    len: u64,
    map: IdMap,
    /// Entries whose 64-bit hash collides with one already in `map`.
    overflow: Vec<(u64, u64)>,
    hits: u64,
    misses: u64,
}

/// One chunk of a shard's node array.
type Chunk = Box<[OnceLock<PNode>]>;

struct Shard {
    index: Mutex<Index>,
    chunks: Box<[OnceLock<Chunk>]>,
}

impl Shard {
    fn new() -> Self {
        Self {
            index: Mutex::new(Index::default()),
            chunks: (0..CHUNKS).map(|_| OnceLock::new()).collect(),
        }
    }

    fn slot(&self, index: u64) -> Option<&OnceLock<PNode>> {
        let (c, off) = locate(index);
        self.chunks.get(c)?.get()?.get(off)
    }

    fn node(&self, index: u64) -> Option<&PNode> {
        self.slot(index)?.get()
    }
}

fn locate(index: u64) -> (usize, usize) {
    let i = index + (1 << BASE_BITS);
    let b = i.ilog2();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "bounded by CHUNKS and the chunk size"
    )]
    ((b - BASE_BITS) as usize, (i - (1 << b)) as usize)
}

/// Intern statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreStats {
    /// Interns that found an existing node.
    pub hits: u64,
    /// Interns that inserted a node.
    pub misses: u64,
    /// Nodes held.
    pub nodes: u64,
    /// Handles held by nodes.
    pub handles: u64,
}

/// The persistent store.
pub struct Store {
    shards: Box<[Shard]>,
    strings: Strings,
    hashcons: bool,
    poisoned: AtomicBool,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("stats", &self.stats())
            .finish()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Store {
    /// A new store. `hashcons: false` is the `-hashcons` ablation: every
    /// intern inserts a fresh node, and equality must be structural.
    #[must_use]
    pub fn new(hashcons: bool) -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Shard::new()).collect(),
            strings: Strings::default(),
            hashcons,
            poisoned: AtomicBool::new(false),
        }
    }

    /// Whether values are hash-consed.
    #[must_use]
    pub fn hashcons(&self) -> bool {
        self.hashcons
    }

    /// The string table.
    #[must_use]
    pub fn strings(&self) -> &Strings {
        &self.strings
    }

    /// Whether a read used a handle the store never issued (a bug).
    #[must_use]
    pub fn poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }

    pub(crate) fn node(&self, h: Handle) -> &PNode {
        let (s, i) = h.persistent_parts();
        if let Some(n) = self.shards.get(usize::from(s)).and_then(|sh| sh.node(i)) {
            n
        } else {
            self.poisoned.store(true, Ordering::Relaxed);
            poison()
        }
    }

    /// Interns a node whose children are all persistent or scalar. Returns
    /// the existing handle when an equal node is present.
    pub(crate) fn intern(
        &self,
        kind: Kind,
        lo: i64,
        hash: u64,
        sum: u64,
        data: &[Handle],
    ) -> Handle {
        #[allow(clippy::cast_possible_truncation, reason = "the top eight bits")]
        let s = (hash >> 56) as u8;
        let Some(shard) = self.shards.get(usize::from(s)) else {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        };
        let mut index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
        if self.hashcons {
            let same = |i: u64| {
                shard.node(i).is_some_and(|n| {
                    n.kind == kind && n.lo == lo && n.hash == hash && *n.data == *data
                })
            };
            if let Some(&i) = index.map.get(&hash) {
                if same(i) {
                    index.hits += 1;
                    return Handle::persistent(s, i);
                }
                if let Some(&(_, j)) = index.overflow.iter().find(|(h, j)| *h == hash && same(*j)) {
                    index.hits += 1;
                    return Handle::persistent(s, j);
                }
            }
        }
        let i = index.len;
        let (c, off) = locate(i);
        let Some(chunk) = shard.chunks.get(c) else {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        };
        let chunk = chunk.get_or_init(|| {
            (0..(1usize << (BASE_BITS as usize + c)))
                .map(|_| OnceLock::new())
                .collect()
        });
        let Some(slot) = chunk.get(off) else {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        };
        if slot
            .set(PNode {
                kind,
                lo,
                hash,
                sum,
                data: data.into(),
            })
            .is_err()
        {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        }
        index.len += 1;
        index.misses += 1;
        if self.hashcons {
            if let std::collections::hash_map::Entry::Vacant(e) = index.map.entry(hash) {
                e.insert(i);
            } else {
                index.overflow.push((hash, i));
            }
        }
        Handle::persistent(s, i)
    }

    /// Intern statistics, summed over the shards.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let mut st = StoreStats::default();
        for shard in &self.shards {
            let index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
            st.hits += index.hits;
            st.misses += index.misses;
            st.nodes += index.len;
            for i in 0..index.len {
                if let Some(n) = shard.node(i) {
                    st.handles += n.data.len() as u64;
                }
            }
        }
        st
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_covers_indices_contiguously() {
        let mut prev = (0usize, 0usize);
        assert_eq!(locate(0), (0, 0));
        for i in 1..5000u64 {
            let (c, off) = locate(i);
            if c == prev.0 {
                assert_eq!(off, prev.1 + 1);
            } else {
                assert_eq!((c, off), (prev.0 + 1, 0));
                assert_eq!(prev.1 + 1, 1 << (BASE_BITS as usize + prev.0));
            }
            prev = (c, off);
        }
    }
}
