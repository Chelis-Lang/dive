//! The persistent, hash-consed, sharded store (`03-value-model.md` §3.1).
//!
//! - 256 shards, selected by the top eight bits of a value's hash.
//! - Node storage is append-only. Each shard has one column per size class;
//!   a column is a chunked vector of `OnceLock` slots that hold a node's
//!   header and its handles inline (the largest class boxes its handles). A
//!   slot is written once, under the shard's lock, and read without a lock,
//!   so reading a node is one chunk load and one slot load.
//! - The deduplication index is an open-addressing table of `u64` entries,
//!   consulted and updated only while interning, under the shard's `Mutex`.
//!   Each shard's lock lives on its own cache line, apart from the columns
//!   that readers touch.
//!
//! All of this is safe Rust. A handle the store did not issue is an internal
//! error: reads return an empty poison view and set [`Store::poisoned`], which
//! the engine reports as a bug, never as a verdict.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::handle::{CLASSES, Handle, LAZY_CLASS, MAX_CLASS_INDEX};
use crate::node::{Kind, View};
use crate::strings::Strings;

const SHARDS: usize = 256;
/// Chunks per column: chunk `k` holds `2^(base + k)` slots, which covers
/// [`MAX_CLASS_INDEX`] for any base.
const CHUNKS: usize = 31;
/// The inline capacities of classes `0..CLASSES - 1`; the last class boxes.
const CAPS: [usize; 2] = [4, 8];

static EMPTY_DATA: [Handle; 0] = [];

const POISON_VIEW: View<'static> = View {
    kind: Kind::Set,
    lo: 0,
    hash: 0,
    sum: 0,
    flags: 0,
    data: &EMPTY_DATA,
};

/// The size class of a node with `len` handles. Lazy nodes (constants
/// only) have a class of their own, so that a handle tells it.
fn class_of(len: usize, fl: u8) -> u8 {
    if fl & crate::node::flags::LAZY != 0 {
        return LAZY_CLASS;
    }
    match len {
        0..=4 => 0,
        5..=8 => 1,
        _ => 2,
    }
}

/// A node's header fields.
#[derive(Clone, Copy)]
struct Head {
    kind: Kind,
    flags: u8,
    lo: i64,
    hash: u64,
    sum: u64,
}

/// A stored node.
trait Slot: Sized {
    fn make(h: Head, data: &[Handle]) -> Self;
    fn view(&self) -> View<'_>;
}

/// A node whose handles are inline (at most `K`).
#[derive(Debug)]
struct Inline<const K: usize> {
    kind: Kind,
    flags: u8,
    len: u8,
    lo: i64,
    hash: u64,
    sum: u64,
    data: [Handle; K],
}

impl<const K: usize> Slot for Inline<K> {
    fn make(
        Head {
            kind,
            flags,
            lo,
            hash,
            sum,
        }: Head,
        data: &[Handle],
    ) -> Self {
        let mut a = [Handle::EMPTY; K];
        let n = data.len().min(K);
        a[..n].copy_from_slice(&data[..n]);
        #[allow(clippy::cast_possible_truncation, reason = "K is at most 8")]
        let len = n as u8;
        Self {
            kind,
            flags,
            len,
            lo,
            hash,
            sum,
            data: a,
        }
    }

    #[inline]
    fn view(&self) -> View<'_> {
        View {
            kind: self.kind,
            lo: self.lo,
            hash: self.hash,
            sum: self.sum,
            flags: self.flags,
            data: self
                .data
                .get(..usize::from(self.len))
                .unwrap_or(&EMPTY_DATA),
        }
    }
}

/// A node of the largest class: its handles in their own allocation.
#[derive(Debug)]
struct Boxed {
    kind: Kind,
    flags: u8,
    lo: i64,
    hash: u64,
    sum: u64,
    data: Box<[Handle]>,
}

impl Slot for Boxed {
    fn make(
        Head {
            kind,
            flags,
            lo,
            hash,
            sum,
        }: Head,
        data: &[Handle],
    ) -> Self {
        Self {
            kind,
            flags,
            lo,
            hash,
            sum,
            data: data.into(),
        }
    }

    #[inline]
    fn view(&self) -> View<'_> {
        View {
            kind: self.kind,
            lo: self.lo,
            hash: self.hash,
            sum: self.sum,
            flags: self.flags,
            data: &self.data,
        }
    }
}

/// The log2 of every column's first chunk (16 slots). One base for all
/// classes lets a read locate its slot before it dispatches on the class.
const BASE: u32 = 4;

/// One column: an append-only chunked vector of write-once slots.
struct Column<T> {
    chunks: [OnceLock<Box<[OnceLock<T>]>>; CHUNKS],
}

impl<T: Slot> Column<T> {
    fn new() -> Self {
        Self {
            chunks: std::array::from_fn(|_| OnceLock::new()),
        }
    }

    #[inline]
    fn get(&self, (c, off): (usize, usize)) -> Option<&T> {
        self.chunks.get(c)?.get()?.get(off)?.get()
    }

    /// Writes slot `index`. Called under the shard's lock, in index order.
    fn put(&self, index: u64, node: T) -> bool {
        let (c, off) = locate(index, BASE);
        let Some(chunk) = self.chunks.get(c) else {
            return false;
        };
        let chunk = chunk.get_or_init(|| {
            (0..(1usize << (BASE as usize + c)))
                .map(|_| OnceLock::new())
                .collect()
        });
        chunk.get(off).is_some_and(|s| s.set(node).is_ok())
    }
}

/// The chunk and offset of slot `index` in a column with first chunk
/// `2^base`.
#[inline]
fn locate(index: u64, base: u32) -> (usize, usize) {
    let i = index + (1 << base);
    let b = i.ilog2();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "bounded by CHUNKS and the chunk size"
    )]
    ((b - base) as usize, (i - (1 << b)) as usize)
}

/// A shard's node storage: one column per size class.
struct Columns {
    c4: Column<Inline<{ CAPS[0] }>>,
    c8: Column<Inline<{ CAPS[1] }>>,
    boxed: Column<Boxed>,
    lazy: Column<Boxed>,
}

impl Columns {
    fn new() -> Self {
        Self {
            c4: Column::new(),
            c8: Column::new(),
            boxed: Column::new(),
            lazy: Column::new(),
        }
    }

    #[allow(
        clippy::inline_always,
        reason = "the hottest read of the core; measured: a call here costs ~5% instructions on MCLamportMutex"
    )]
    #[inline(always)]
    fn view(&self, class: u8, index: u64) -> Option<View<'_>> {
        let at = locate(index, BASE);
        if class == 0 {
            self.c4.get(at).map(Slot::view)
        } else if class == 1 {
            self.c8.get(at).map(Slot::view)
        } else if class == 2 {
            self.boxed.get(at).map(Slot::view)
        } else {
            self.lazy.get(at).map(Slot::view)
        }
    }

    fn put(&self, class: u8, index: u64, h: Head, d: &[Handle]) -> bool {
        match class {
            0 => self.c4.put(index, Slot::make(h, d)),
            1 => self.c8.put(index, Slot::make(h, d)),
            2 => self.boxed.put(index, Slot::make(h, d)),
            3 => self.lazy.put(index, Slot::make(h, d)),
            _ => false,
        }
    }
}

/// The deduplication index of one shard.
///
/// `table` is open addressing with linear probing. An entry is
/// `(low 32 bits of the hash) << 32 | (local + 1)`, where `local` is
/// `class << 29 | index`; zero is empty. The probe position is the low bits
/// of the hash, so growing needs no node reads.
#[derive(Default)]
struct Index {
    lens: [u64; CLASSES],
    table: Vec<u64>,
    used: usize,
    hits: u64,
    misses: u64,
    handles: u64,
}

const LOCAL_CLASS_SHIFT: u32 = 29;

impl Index {
    fn grow(&mut self) {
        let cap = (self.table.len() * 2).max(64);
        let mut t = vec![0u64; cap];
        let mask = cap - 1;
        for &e in &self.table {
            if e != 0 {
                let mut p = (e >> 32) as usize & mask;
                while t[p] != 0 {
                    p = (p + 1) & mask;
                }
                t[p] = e;
            }
        }
        self.table = t;
    }
}

/// One shard's lock, on its own cache line.
#[repr(align(128))]
struct Shard {
    index: Mutex<Index>,
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
    /// Nodes per size class (inline capacity 4, 8, then boxed, then lazy
    /// constants).
    pub classes: [u64; CLASSES],
}

/// The persistent store.
pub struct Store {
    cols: Box<[Columns]>,
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
            cols: (0..SHARDS).map(|_| Columns::new()).collect(),
            shards: (0..SHARDS)
                .map(|_| Shard {
                    index: Mutex::new(Index::default()),
                })
                .collect(),
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

    /// The node behind a persistent handle. A handle the store did not issue
    /// gives an empty set view and poisons the store.
    #[inline]
    pub(crate) fn view(&self, h: Handle) -> View<'_> {
        let (s, class, i) = h.persistent_parts();
        if let Some(v) = self.cols.get(usize::from(s)).and_then(|c| c.view(class, i)) {
            v
        } else {
            self.poisoned.store(true, Ordering::Relaxed);
            POISON_VIEW
        }
    }

    /// Interns a node whose children are all persistent or scalar. Returns
    /// the existing handle when an equal node is present.
    #[allow(clippy::too_many_arguments, reason = "the node header, flat")]
    pub(crate) fn intern(
        &self,
        kind: Kind,
        flags: u8,
        lo: i64,
        hash: u64,
        sum: u64,
        data: &[Handle],
    ) -> Handle {
        #[allow(clippy::cast_possible_truncation, reason = "the top eight bits")]
        let s = (hash >> 56) as u8;
        let (Some(shard), Some(cols)) = (
            self.shards.get(usize::from(s)),
            self.cols.get(usize::from(s)),
        ) else {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        };
        let mut index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
        #[allow(clippy::cast_possible_truncation, reason = "the low 32 bits")]
        let h32 = hash as u32;
        let mut slot = None;
        if self.hashcons {
            if (index.used + 1) * 4 > index.table.len() * 3 {
                index.grow();
            }
            let mask = index.table.len() - 1;
            let mut p = h32 as usize & mask;
            loop {
                let e = index.table[p];
                if e == 0 {
                    slot = Some(p);
                    break;
                }
                #[allow(clippy::cast_possible_truncation, reason = "packed entry")]
                if (e >> 32) as u32 == h32 {
                    let local = (e as u32).wrapping_sub(1);
                    #[allow(clippy::cast_possible_truncation, reason = "three class bits")]
                    let class = (local >> LOCAL_CLASS_SHIFT) as u8;
                    let i = u64::from(local & ((1 << LOCAL_CLASS_SHIFT) - 1));
                    if cols.view(class, i).is_some_and(|n| {
                        n.kind == kind && n.lo == lo && n.hash == hash && n.data == data
                    }) {
                        index.hits += 1;
                        return Handle::persistent(s, class, i);
                    }
                }
                p = (p + 1) & mask;
            }
        }
        let class = class_of(data.len(), flags);
        let i = index.lens[usize::from(class)];
        if i >= MAX_CLASS_INDEX
            || !cols.put(
                class,
                i,
                Head {
                    kind,
                    flags,
                    lo,
                    hash,
                    sum,
                },
                data,
            )
        {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        }
        index.lens[usize::from(class)] += 1;
        index.misses += 1;
        index.handles += data.len() as u64;
        if let Some(p) = slot {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "bounded by MAX_CLASS_INDEX"
            )]
            let local = (u32::from(class) << LOCAL_CLASS_SHIFT) | i as u32;
            index.table[p] = (u64::from(h32) << 32) | u64::from(local + 1);
            index.used += 1;
        }
        Handle::persistent(s, class, i)
    }

    /// Intern statistics, summed over the shards.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let mut st = StoreStats::default();
        for shard in &self.shards {
            let index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
            st.hits += index.hits;
            st.misses += index.misses;
            st.handles += index.handles;
            for (c, &n) in index.lens.iter().enumerate() {
                st.nodes += n;
                st.classes[c] += n;
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
        for base in [4u32, 6, 8] {
            let mut prev = (0usize, 0usize);
            assert_eq!(locate(0, base), (0, 0));
            for i in 1..5000u64 {
                let (c, off) = locate(i, base);
                if c == prev.0 {
                    assert_eq!(off, prev.1 + 1);
                } else {
                    assert_eq!((c, off), (prev.0 + 1, 0));
                    assert_eq!(prev.1 + 1, 1 << (base as usize + prev.0));
                }
                prev = (c, off);
            }
            let (c, _) = locate(MAX_CLASS_INDEX - 1, base);
            assert!(c < CHUNKS, "the last index fits for base {base}");
        }
    }

    fn int(v: i64) -> Handle {
        Handle::int(v).unwrap_or(Handle::EMPTY)
    }

    #[test]
    fn every_size_class_round_trips_and_dedupes() {
        let store = Store::default();
        let mut got = Vec::new();
        for len in [0i64, 1, 4, 5, 8, 9, 16, 17, 32, 33, 100] {
            let data: Vec<Handle> = (0..len).map(int).collect();
            let hash = crate::handle::mix64(len.unsigned_abs() + 1);
            let h = store.intern(Kind::Set, 0, 7, hash, 3, &data);
            let v = store.view(h);
            assert_eq!(v.data, data.as_slice(), "len {len}");
            assert_eq!((v.kind, v.lo, v.hash, v.sum), (Kind::Set, 7, hash, 3));
            assert_eq!(
                store.intern(Kind::Set, 0, 7, hash, 3, &data),
                h,
                "len {len}"
            );
            got.push(h);
        }
        let st = store.stats();
        assert_eq!(st.nodes, got.len() as u64);
        assert_eq!(st.hits, got.len() as u64);
        assert_eq!(st.classes, [3, 2, 6, 0]);
        assert!(!store.poisoned());
    }

    #[test]
    fn colliding_hashes_are_told_apart_and_the_table_grows() {
        let store = Store::default();
        // Every node has the same hash: probing must compare contents.
        let hash = 0xabcd_0000_0000_1234;
        let hs: Vec<Handle> = (0..500)
            .map(|i| store.intern(Kind::Set, 0, 0, hash, 0, &[int(i)]))
            .collect();
        for (i, &h) in (0..).zip(&hs) {
            assert_eq!(store.view(h).data, &[int(i)]);
            assert_eq!(store.intern(Kind::Set, 0, 0, hash, 0, &[int(i)]), h);
        }
        // Distinct hashes in one shard, enough to grow the table repeatedly.
        for i in 0..5000u64 {
            let hash = (0x11u64 << 56) | crate::handle::mix64(i) >> 8;
            let a = store.intern(Kind::Set, 0, 0, hash, 0, &[int(1)]);
            assert_eq!(store.intern(Kind::Set, 0, 0, hash, 0, &[int(1)]), a);
        }
        assert_eq!(store.stats().nodes, 5500);
        assert!(!store.poisoned());
    }

    #[test]
    fn without_hashconsing_every_intern_inserts() {
        let store = Store::new(false);
        let a = store.intern(Kind::Set, 0, 0, 9, 9, &[int(1)]);
        let b = store.intern(Kind::Set, 0, 0, 9, 9, &[int(1)]);
        assert_ne!(a, b);
        assert_eq!(store.view(a).data, store.view(b).data);
        assert_eq!(store.stats().nodes, 2);
    }

    #[test]
    fn foreign_handles_poison_the_store() {
        let store = Store::default();
        let v = store.view(Handle::persistent(3, 0, 99));
        assert_eq!(v.data.len(), 0);
        assert!(store.poisoned());
    }
}
