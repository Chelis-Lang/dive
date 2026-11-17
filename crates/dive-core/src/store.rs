//! The persistent, hash-consed, sharded store.
//!
//! - 256 shards, selected by the top eight bits of the interning hash. For
//!   retained construction views this includes their exact representation;
//!   canonical nodes use their value hash unchanged.
//! - Node storage is append-only. Each shard has one column per size class;
//!   a column is a chunked vector of `OnceLock` slots that hold a node's
//!   header and its handles inline (the largest class boxes its handles). A
//!   slot is written once, under the shard's lock, and read without a lock,
//!   so reading a node is one chunk load and one slot load.
//! - The deduplication index is an open-addressing table of atomic `u64`
//!   entries. An intern that finds its node (a hit, by far the common case)
//!   probes it without a lock, through the interning heap's
//!   [`InternCache`] of the shard's current table; a miss takes the shard's
//!   `Mutex`, probes again, and inserts. Growing replaces the table and
//!   bumps the shard's generation, so a cached table is refreshed (under
//!   the lock) when the generation moves; an old table stays valid for the
//!   readers that still hold it, because nodes never move.
//!   Each shard's lock and generation live on their own cache lines, apart
//!   from the columns that readers touch.
//!
//! All of this is safe Rust. A handle the store did not issue is an internal
//! error: reads return an empty poison view and set [`Store::poisoned`], which
//! the engine reports as a bug, never as a verdict.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use crate::handle::{CLASSES, Handle, LAZY_CLASS, MAX_CLASS_INDEX, mix2};
use crate::node::{Kind, View, flags};
use crate::strings::{StringOrder, Strings};

pub mod compact;
mod memory;
pub mod snapshot;
pub use memory::StoreMemory;

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
    if fl & (crate::node::flags::LAZY | crate::node::flags::VIEW) != 0 {
        return LAZY_CLASS;
    }
    match len {
        0..=4 => 0,
        5..=8 => 1,
        _ => 2,
    }
}

/// A view parent has an extensional value hash, but exact interning must
/// distinguish its retained child handles. Using only the value hash puts all
/// equal independent representations in one probe cluster, making ordinary
/// state handoff quadratic. Keep that representation key private to the
/// index: value hashes and canonical interning stay intact.
fn interning_hash(kind: Kind, fl: u8, lo: i64, hash: u64, data: &[Handle]) -> u64 {
    if fl & flags::NESTED_VIEW == 0 {
        return hash;
    }
    let header = u64::from(kind as u8) | (u64::from(fl & !flags::CONCRETE) << 8);
    let hash = mix2(mix2(hash, header), lo.cast_unsigned());
    data.iter().fold(hash, |key, h| mix2(key, h.bits()))
}

/// Whether a stored node is the one being interned. [`flags::CONCRETE`] is a
/// cached proof, not identity, so it is ignored.
fn same_identity(n: &View<'_>, kind: Kind, fl: u8, lo: i64, hash: u64, data: &[Handle]) -> bool {
    n.kind == kind
        && (n.flags & !flags::CONCRETE) == (fl & !flags::CONCRETE)
        && n.lo == lo
        && n.hash == hash
        && n.data == data
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

    fn allocated_bytes(&self) -> usize {
        self.chunks
            .iter()
            .filter_map(OnceLock::get)
            .map(|chunk| std::mem::size_of_val(chunk.as_ref()))
            .sum()
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

/// The class and index a table entry names.
fn unpack(e: u64) -> (u8, u64) {
    #[allow(clippy::cast_possible_truncation, reason = "packed entry")]
    let local = (e as u32).wrapping_sub(1);
    #[allow(clippy::cast_possible_truncation, reason = "three class bits")]
    let class = (local >> LOCAL_CLASS_SHIFT) as u8;
    (class, u64::from(local & ((1 << LOCAL_CLASS_SHIFT) - 1)))
}

/// Probes `table` for a node with the low hash bits `h32` that `same`
/// accepts, without a lock. `None` is a miss in this table only: an entry
/// written meanwhile, or into a newer table, is found under the lock.
#[inline]
fn probe(table: &[AtomicU64], h32: u32, same: impl Fn(u8, u64) -> bool) -> Option<(u8, u64)> {
    if table.is_empty() {
        return None;
    }
    let mask = table.len() - 1;
    let mut p = h32 as usize & mask;
    loop {
        let e = table[p].load(Ordering::Acquire);
        if e == 0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, reason = "packed entry")]
        if (e >> 32) as u32 == h32 {
            let (class, i) = unpack(e);
            if same(class, i) {
                return Some((class, i));
            }
        }
        p = (p + 1) & mask;
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
/// `(low 32 bits of the interning hash) << 32 | (local + 1)`, where `local` is
/// `class << 29 | index`; zero is empty. The probe position is the low bits
/// of the hash, so growing needs no node reads.
struct Index {
    lens: [u64; CLASSES],
    table: Table,
    used: usize,
    hits: u64,
    misses: u64,
    handles: u64,
    boxed_payload_bytes: usize,
    retired: Vec<RetiredTable>,
    rehash_scratch_bytes: usize,
    largest_rehash_scratch_bytes: usize,
}

/// A shard's deduplication table. Entries are written once, under the
/// shard's lock, after the node they name; readers load them without it.
type Table = Arc<[AtomicU64]>;

/// Weak records do not keep old table payload alive. They let accounting see
/// tables held by `InternCache` readers after the index replaces its own Arc.
struct RetiredTable {
    table: Weak<[AtomicU64]>,
}

fn empty_table(cap: usize) -> Table {
    (0..cap).map(|_| AtomicU64::new(0)).collect()
}

const LOCAL_CLASS_SHIFT: u32 = 29;

impl Default for Index {
    fn default() -> Self {
        Self {
            lens: [0; CLASSES],
            table: empty_table(0),
            used: 0,
            hits: 0,
            misses: 0,
            handles: 0,
            boxed_payload_bytes: 0,
            retired: Vec::new(),
            rehash_scratch_bytes: 0,
            largest_rehash_scratch_bytes: 0,
        }
    }
}

impl Index {
    /// Doubles the table into a new one; the old one stays valid for the
    /// readers that hold it.
    fn grow(&mut self) {
        let cap = (self.table.len() * 2).max(64);
        let t: Vec<u64> = {
            let mut t = vec![0u64; cap];
            let mask = cap - 1;
            for e in self.table.iter() {
                let e = e.load(Ordering::Relaxed);
                if e != 0 {
                    let mut p = (e >> 32) as usize & mask;
                    while t[p] != 0 {
                        p = (p + 1) & mask;
                    }
                    t[p] = e;
                }
            }
            t
        };
        let scratch = t.capacity() * std::mem::size_of::<u64>();
        self.rehash_scratch_bytes += scratch;
        self.largest_rehash_scratch_bytes = self.largest_rehash_scratch_bytes.max(scratch);
        if !self.table.is_empty() && Arc::strong_count(&self.table) > 1 {
            self.retired.push(RetiredTable {
                table: Arc::downgrade(&self.table),
            });
        }
        self.table = t.into_iter().map(AtomicU64::new).collect();
    }

    fn memory_usage(&mut self, out: &mut StoreMemory) {
        out.boxed_payload_bytes += self.boxed_payload_bytes;
        out.current_table_bytes += std::mem::size_of_val(self.table.as_ref());
        // Upgrading pins the payload while it is measured; a second reader does
        // not multiply its allocation. Dead records release their Weak metadata.
        self.retired.retain(|retired| {
            if let Some(table) = retired.table.upgrade() {
                out.retired_table_bytes += std::mem::size_of_val(table.as_ref());
                out.retired_tables += 1;
                true
            } else {
                false
            }
        });
        out.retirement_tracking_bytes +=
            self.retired.capacity() * std::mem::size_of::<RetiredTable>();
        out.rehash_scratch_bytes += self.rehash_scratch_bytes;
        out.largest_rehash_scratch_bytes = out
            .largest_rehash_scratch_bytes
            .max(self.largest_rehash_scratch_bytes);
    }
}

/// One shard's lock, on its own cache line.
#[repr(align(128))]
struct Shard {
    index: Mutex<Index>,
}

/// One shard's table generation, on its own cache line: bumped, under the
/// lock, each time the table is replaced.
#[repr(align(128))]
#[derive(Default)]
struct Generation(AtomicU64);

/// A counter on its own cache line.
#[repr(align(128))]
#[derive(Default)]
struct Padded(AtomicU64);

/// Stripes of the lock-free hit counter.
const HIT_STRIPES: usize = 16;

/// A heap's view of the store's deduplication tables, for interning
/// without the shard locks: the table of each shard it has
/// interned into, with the generation it was read at, and the hits it has
/// not yet added to the store's statistics.
pub struct InternCache {
    tables: Vec<Option<(u64, Table)>>,
    hits: u64,
    stripe: usize,
    /// About the bytes of the nodes this heap has added to the store (a
    /// node header and its handles each): what a caller that must bound
    /// its persistent copies, such as a primed-definition memo site,
    /// charges (`Machine::memo_eval`).
    pub(crate) added: u64,
}

impl Default for InternCache {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "reduced modulo the stripes"
        )]
        let stripe = (NEXT.fetch_add(1, Ordering::Relaxed) as usize) % HIT_STRIPES;
        Self {
            tables: vec![None; SHARDS],
            hits: 0,
            stripe,
            added: 0,
        }
    }
}

impl std::fmt::Debug for InternCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternCache")
            .field("hits", &self.hits)
            .finish_non_exhaustive()
    }
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
    generations: Box<[Generation]>,
    hits: Box<[Padded]>,
    strings: Strings,
    hashcons: bool,
    semantics: Semantics,
    poisoned: AtomicBool,
    /// The model's names, for printing ([`Store::set_printer`]).
    printer: OnceLock<crate::print::Printer>,
    /// Whether the plan may build an interval or a lazy set
    /// ([`Store::lazy_sets`]).
    lazy_sets: AtomicBool,
    /// The run's random streams ([`Store::set_draws`]).
    draws: OnceLock<std::sync::Arc<dyn crate::random::Draws>>,
    /// The run's registers ([`Store::set_registers`]).
    registers: OnceLock<std::sync::Arc<dyn crate::random::Registers>>,
}

/// The value semantics of a run, fixed for the store's life: the default
/// profile's, or `--compat=tlc`'s ([`Semantics::tlc`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Semantics {
    /// How strings compare.
    pub string_order: StringOrder,
    /// The integer semantics every heap on the store starts with.
    pub int_mode: crate::int::IntMode,
    /// TLC's comparisons (`--comparisons=tlc`) rather than TLA+'s: where
    /// TLC's `equals`, `compareTo` or `member` may fail or answer otherwise
    /// than TLA+, the core refuses (`compare.rs`).
    pub tlc_comparisons: bool,
    /// TLC's exploration order (`--explore-order=tlc`, A6): `x' \in S`
    /// binds in the order of TLC's `elements()`, which the core follows
    /// where it is the normalized order and refuses elsewhere.
    pub tlc_exploration: bool,
}

impl Semantics {
    /// `--compat=tlc`: token string order, 32-bit integers, TLC's
    /// comparisons.
    #[must_use]
    pub const fn tlc() -> Self {
        Self {
            string_order: StringOrder::Token,
            int_mode: crate::int::IntMode::Tlc,
            tlc_comparisons: true,
            tlc_exploration: true,
        }
    }
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
        Self::with_string_order(hashcons, StringOrder::Text)
    }

    /// A new store whose strings compare in `order` (`--compat=tlc` uses
    /// [`StringOrder::Token`]). The order is fixed for the store's life,
    /// because sets are kept sorted in it.
    #[must_use]
    pub fn with_string_order(hashcons: bool, order: StringOrder) -> Self {
        Self::with_semantics(
            hashcons,
            Semantics {
                string_order: order,
                ..Semantics::default()
            },
        )
    }

    /// A new store with the run's value semantics, so that constant
    /// folding, the search and trace replay all compute alike.
    #[must_use]
    pub fn with_semantics(hashcons: bool, semantics: Semantics) -> Self {
        Self {
            cols: (0..SHARDS).map(|_| Columns::new()).collect(),
            shards: (0..SHARDS)
                .map(|_| Shard {
                    index: Mutex::new(Index::default()),
                })
                .collect(),
            generations: (0..SHARDS).map(|_| Generation::default()).collect(),
            hits: (0..HIT_STRIPES).map(|_| Padded::default()).collect(),
            strings: Strings::new(semantics.string_order),
            hashcons,
            semantics,
            poisoned: AtomicBool::new(false),
            printer: OnceLock::new(),
            lazy_sets: AtomicBool::new(false),
            draws: OnceLock::new(),
            registers: OnceLock::new(),
        }
    }

    /// Sets the model's name table and value semantics, which printing
    /// (`ToString`, `Print`) converts values with (`print.rs`). The first
    /// call wins.
    pub fn set_printer(&self, printer: crate::print::Printer) {
        let _ = self.printer.set(printer);
    }

    /// Records that the plan may build a value TLC keeps as an interval or
    /// a lazy set (`a..b`, `DOMAIN`, `SUBSET`, `[S -> T]`, `[a : S]`,
    /// `S \X T`), which the core may enumerate: printing then refuses the
    /// enumerated sets TLC could print otherwise (`print.rs`). Set while
    /// compiling, before any search.
    pub fn note_lazy_sets(&self) {
        self.lazy_sets.store(true, Ordering::Relaxed);
    }

    /// Whether [`Store::note_lazy_sets`] was called.
    #[must_use]
    pub fn lazy_sets(&self) -> bool {
        self.lazy_sets.load(Ordering::Relaxed)
    }

    /// Sets the run's random streams (`RandomElement`). The first call
    /// wins.
    pub fn set_draws(&self, draws: std::sync::Arc<dyn crate::random::Draws>) {
        let _ = self.draws.set(draws);
    }

    /// The run's random streams, once set.
    #[must_use]
    pub fn draws(&self) -> Option<&std::sync::Arc<dyn crate::random::Draws>> {
        self.draws.get()
    }

    /// Sets the run's `TLCSet`/`TLCGet` registers. The first call wins.
    pub fn set_registers(&self, registers: std::sync::Arc<dyn crate::random::Registers>) {
        let _ = self.registers.set(registers);
    }

    /// The run's registers, once set.
    #[must_use]
    pub fn registers(&self) -> Option<&std::sync::Arc<dyn crate::random::Registers>> {
        self.registers.get()
    }

    /// The printer, once set.
    #[must_use]
    pub fn printer(&self) -> Option<&crate::print::Printer> {
        self.printer.get()
    }

    /// Whether values are hash-consed.
    #[must_use]
    pub fn hashcons(&self) -> bool {
        self.hashcons
    }

    /// The run's value semantics.
    #[must_use]
    pub fn semantics(&self) -> Semantics {
        self.semantics
    }

    /// The integer semantics heaps on this store start with.
    #[must_use]
    pub fn int_mode(&self) -> crate::int::IntMode {
        self.semantics.int_mode
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

    /// Validates an opaque coordinate for the live-Store frontier codec without
    /// turning corrupt file bytes into a poison read or a fabricated value.
    pub(crate) fn retained_view(&self, handle: Handle) -> Option<View<'_>> {
        if !handle.is_persistent() {
            return None;
        }
        let (shard, class, index) = handle.persistent_parts();
        if index > MAX_CLASS_INDEX {
            return None;
        }
        self.cols
            .get(usize::from(shard))
            .and_then(|c| c.view(class, index))
    }

    /// Interns a node whose children are all persistent or scalar. Returns
    /// the existing handle when an equal node is present. `cache` is the
    /// interning heap's: a hit is found without the shard's lock.
    #[allow(clippy::too_many_arguments, reason = "the node header, flat")]
    pub(crate) fn intern(
        &self,
        cache: &mut InternCache,
        kind: Kind,
        flags: u8,
        lo: i64,
        hash: u64,
        sum: u64,
        data: &[Handle],
    ) -> Handle {
        let index_hash = interning_hash(kind, flags, lo, hash, data);
        #[allow(clippy::cast_possible_truncation, reason = "the top eight bits")]
        let s = (index_hash >> 56) as u8;
        let (Some(shard), Some(cols), Some(generation)) = (
            self.shards.get(usize::from(s)),
            self.cols.get(usize::from(s)),
            self.generations.get(usize::from(s)),
        ) else {
            self.poisoned.store(true, Ordering::Relaxed);
            return Handle::EMPTY;
        };
        #[allow(clippy::cast_possible_truncation, reason = "the low 32 bits")]
        let h32 = index_hash as u32;
        let same = |class: u8, i: u64| {
            cols.view(class, i)
                .is_some_and(|n| same_identity(&n, kind, flags, lo, hash, data))
        };
        if self.hashcons {
            // The lock-free probe of the cached table.
            let g = generation.0.load(Ordering::Acquire);
            let entry = cache.tables.get_mut(usize::from(s));
            if let Some(entry) = entry {
                if entry.as_ref().is_none_or(|(cg, _)| *cg != g) {
                    let index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
                    *entry = Some((
                        generation.0.load(Ordering::Relaxed),
                        Arc::clone(&index.table),
                    ));
                }
                if let Some((_, table)) = entry.as_ref()
                    && let Some(found) = probe(table, h32, same)
                {
                    cache.hits += 1;
                    let (class, i) = found;
                    return Handle::persistent(s, class, i);
                }
            }
        }
        let mut index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
        let mut slot = None;
        if self.hashcons {
            if (index.used + 1) * 4 > index.table.len() * 3 {
                index.grow();
                generation.0.fetch_add(1, Ordering::Release);
            }
            let mask = index.table.len() - 1;
            let mut p = h32 as usize & mask;
            loop {
                let e = index.table[p].load(Ordering::Relaxed);
                if e == 0 {
                    slot = Some(p);
                    break;
                }
                #[allow(clippy::cast_possible_truncation, reason = "packed entry")]
                if (e >> 32) as u32 == h32 {
                    let (class, i) = unpack(e);
                    if same(class, i) {
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
        cache.added += (std::mem::size_of::<Head>() + std::mem::size_of_val(data)) as u64;
        index.handles += data.len() as u64;
        index.boxed_payload_bytes += usize::from(class >= 2) * std::mem::size_of_val(data);
        if let Some(p) = slot {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "bounded by MAX_CLASS_INDEX"
            )]
            let local = (u32::from(class) << LOCAL_CLASS_SHIFT) | i as u32;
            // After the node: a reader that sees the entry sees the node.
            index.table[p].store(
                (u64::from(h32) << 32) | u64::from(local + 1),
                Ordering::Release,
            );
            index.used += 1;
        }
        Handle::persistent(s, class, i)
    }

    /// Adds a heap's lock-free hits to the statistics.
    pub(crate) fn flush_hits(&self, cache: &mut InternCache) {
        if cache.hits != 0 {
            if let Some(c) = self.hits.get(cache.stripe) {
                c.0.fetch_add(cache.hits, Ordering::Relaxed);
            }
            cache.hits = 0;
        }
    }

    /// Intern statistics, summed over the shards.
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let mut st = StoreStats::default();
        for c in &self.hits {
            st.hits += c.0.load(Ordering::Relaxed);
        }
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

    /// Measures known requested retained bytes and capacities, including tables
    /// still held by readers after index growth. Dead retirement records are
    /// collected before reporting; their vector's spare capacity still counts.
    ///
    /// This read takes shard locks, without changing interning identities or
    /// hit/miss counters. It is intended for phase boundaries, not the hot path.
    /// See [`StoreMemory`] for opaque allocations and snapshot limitations.
    ///
    /// ```
    /// let store = dive_core::Store::default();
    /// let memory = store.memory_usage();
    /// assert!(memory.fixed_bytes > 0);
    /// assert_eq!(memory.slab_bytes, 0);
    /// assert!(memory.known_retained_bytes().is_some());
    /// ```
    #[must_use]
    pub fn memory_usage(&self) -> StoreMemory {
        let mut out = StoreMemory {
            fixed_bytes: std::mem::size_of::<Self>()
                + std::mem::size_of_val(self.cols.as_ref())
                + std::mem::size_of_val(self.shards.as_ref())
                + std::mem::size_of_val(self.generations.as_ref())
                + std::mem::size_of_val(self.hits.as_ref()),
            strings: self.strings.memory_usage(),
            has_printer: self.printer.get().is_some(),
            has_draws: self.draws.get().is_some(),
            has_registers: self.registers.get().is_some(),
            ..StoreMemory::default()
        };
        for (cols, shard) in self.cols.iter().zip(&self.shards) {
            let mut index = shard.index.lock().unwrap_or_else(PoisonError::into_inner);
            out.slab_bytes += cols.c4.allocated_bytes()
                + cols.c8.allocated_bytes()
                + cols.boxed.allocated_bytes()
                + cols.lazy.allocated_bytes();
            index.memory_usage(&mut out);
        }
        out
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
    fn memory_counts_allocated_slabs_and_actual_boxed_payload() {
        let store = Store::new(false);
        let mut cache = InternCache::default();
        let empty = store.memory_usage();
        assert!(empty.fixed_bytes > 0);
        assert_eq!(empty.slab_bytes, 0);
        for i in 0..17 {
            let _ = store.intern(&mut cache, Kind::Set, 0, 0, 0, 0, &[int(i)]);
        }
        let inline = store.memory_usage();
        // Seventeen occupied slots retain both whole chunks: 16 + 32 slots.
        assert_eq!(
            inline.slab_bytes,
            48 * std::mem::size_of::<OnceLock<Inline<4>>>()
        );
        assert_eq!(inline.boxed_payload_bytes, 0);
        let five = [int(0); 5];
        let nine = [int(0); 9];
        let two = [int(0); 2];
        let _ = store.intern(&mut cache, Kind::Set, 0, 0, 0, 0, &five);
        let _ = store.intern(&mut cache, Kind::Set, 0, 0, 0, 0, &nine);
        let _ = store.intern(
            &mut cache,
            Kind::Fcn,
            crate::node::flags::RECORD,
            0,
            0,
            0,
            &two,
        );
        let all = store.memory_usage();
        assert_eq!(all.fixed_bytes, empty.fixed_bytes);
        assert_eq!(
            all.slab_bytes,
            inline.slab_bytes
                + 16 * std::mem::size_of::<OnceLock<Inline<8>>>()
                + 32 * std::mem::size_of::<OnceLock<Boxed>>()
        );
        assert_eq!(all.boxed_payload_bytes, 11 * std::mem::size_of::<Handle>());
        assert_eq!(
            all.current_table_bytes, 0,
            "hashcons-off has no index tables"
        );
        assert_eq!(all.retired_tables, 0);
        assert_eq!(all.rehash_scratch_bytes, 0);
        assert!(all.known_retained_bytes() > inline.known_retained_bytes());
    }

    #[test]
    fn memory_counts_shared_retired_tables_until_the_last_reader_drops() {
        let store = Store::default();
        let hash = 0xabcd_0000_0000_1234;
        let mut first = InternCache::default();
        let mut second = InternCache::default();
        let mut writer = InternCache::default();
        let initial = store.intern(&mut first, Kind::Set, 0, 0, hash, 0, &[int(0)]);
        assert_eq!(
            store.intern(&mut first, Kind::Set, 0, 0, hash, 0, &[int(0)]),
            initial
        );
        assert_eq!(
            store.intern(&mut second, Kind::Set, 0, 0, hash, 0, &[int(0)]),
            initial
        );
        for i in 1..49 {
            let _ = store.intern(&mut writer, Kind::Set, 0, 0, hash, 0, &[int(i)]);
        }
        let memory = store.memory_usage();
        assert_eq!(memory.current_table_bytes, 128 * std::mem::size_of::<u64>());
        assert_eq!(memory.retired_table_bytes, 64 * std::mem::size_of::<u64>());
        assert_eq!(
            memory.retired_tables, 1,
            "three readers share one allocation"
        );
        assert!(memory.retirement_tracking_bytes >= std::mem::size_of::<RetiredTable>());
        assert_eq!(
            memory.rehash_scratch_bytes,
            192 * std::mem::size_of::<u64>()
        );
        assert_eq!(
            memory.largest_rehash_scratch_bytes,
            memory.current_table_bytes
        );
        // Measuring is observational: counters and identities do not change.
        let stats = store.stats();
        assert_eq!(store.memory_usage(), memory);
        assert_eq!(store.stats(), stats);
        drop(writer);
        drop(first);
        assert_eq!(
            store.memory_usage().retired_table_bytes,
            memory.retired_table_bytes
        );
        drop(second);
        let released = store.memory_usage();
        assert_eq!(released.retired_table_bytes, 0);
        assert_eq!(released.retired_tables, 0);
        assert_eq!(released.current_table_bytes, memory.current_table_bytes);
        assert_eq!(
            released.retirement_tracking_bytes,
            memory.retirement_tracking_bytes
        );
        let shard = &store.shards[0xab];
        assert!(
            shard
                .index
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retired
                .is_empty(),
            "dead Weak records are removed, without hiding spare Vec capacity"
        );
    }

    #[test]
    fn memory_does_not_charge_hashcons_hits_as_new_payload() {
        let store = Store::default();
        let mut cache = InternCache::default();
        let data = [int(1); 12];
        let original = store.intern(&mut cache, Kind::Set, 0, 0, 19, 0, &data);
        let before = store.memory_usage();
        for _ in 0..20 {
            assert_eq!(
                store.intern(&mut cache, Kind::Set, 0, 0, 19, 0, &data),
                original
            );
        }
        assert_eq!(store.memory_usage(), before);
        assert_eq!(before.boxed_payload_bytes, std::mem::size_of_val(&data));
        assert_eq!(store.stats().nodes, 1);
    }

    #[test]
    fn every_size_class_round_trips_and_dedupes() {
        let store = Store::default();
        let mut c = InternCache::default();
        let mut got = Vec::new();
        for len in [0i64, 1, 4, 5, 8, 9, 16, 17, 32, 33, 100] {
            let data: Vec<Handle> = (0..len).map(int).collect();
            let hash = crate::handle::mix64(len.unsigned_abs() + 1);
            let h = store.intern(&mut c, Kind::Set, 0, 7, hash, 3, &data);
            let v = store.view(h);
            assert_eq!(v.data, data.as_slice(), "len {len}");
            assert_eq!((v.kind, v.lo, v.hash, v.sum), (Kind::Set, 7, hash, 3));
            assert_eq!(
                store.intern(&mut c, Kind::Set, 0, 7, hash, 3, &data),
                h,
                "len {len}"
            );
            got.push(h);
        }
        store.flush_hits(&mut c);
        let st = store.stats();
        assert_eq!(st.nodes, got.len() as u64);
        assert_eq!(st.hits, got.len() as u64);
        assert_eq!(st.classes, [3, 2, 6, 0]);
        assert!(!store.poisoned());
    }

    #[test]
    fn colliding_hashes_are_told_apart_and_the_table_grows() {
        let store = Store::default();
        let mut c = InternCache::default();
        // Every node has the same hash: probing must compare contents.
        let hash = 0xabcd_0000_0000_1234;
        let hs: Vec<Handle> = (0..500)
            .map(|i| store.intern(&mut c, Kind::Set, 0, 0, hash, 0, &[int(i)]))
            .collect();
        for (i, &h) in (0..).zip(&hs) {
            assert_eq!(store.view(h).data, &[int(i)]);
            assert_eq!(store.intern(&mut c, Kind::Set, 0, 0, hash, 0, &[int(i)]), h);
        }
        // Distinct hashes in one shard, enough to grow the table repeatedly.
        for i in 0..5000u64 {
            let hash = (0x11u64 << 56) | crate::handle::mix64(i) >> 8;
            let a = store.intern(&mut c, Kind::Set, 0, 0, hash, 0, &[int(1)]);
            assert_eq!(store.intern(&mut c, Kind::Set, 0, 0, hash, 0, &[int(1)]), a);
        }
        assert_eq!(store.stats().nodes, 5500);
        assert!(!store.poisoned());
    }

    #[test]
    fn concurrent_interns_dedupe_while_tables_grow() {
        // Every thread interns the same values, in different orders, so
        // that hits race inserts and growth, and every lock-free probe
        // either finds the node or falls back to the lock.
        let store = std::sync::Arc::new(Store::default());
        let threads: Vec<_> = (0..8u64)
            .map(|t| {
                let store = std::sync::Arc::clone(&store);
                std::thread::spawn(move || {
                    let mut c = InternCache::default();
                    let mut out = vec![Handle::EMPTY; 20_011];
                    for k in 0..20_011u64 {
                        let i = (k * (2 * t + 1)) % 20_011; // prime, so a permutation
                        let hash = crate::handle::mix64(i);
                        let v = i64::try_from(i).unwrap_or(0);
                        out[usize::try_from(i).unwrap_or(0)] =
                            store.intern(&mut c, Kind::Set, 0, 0, hash, 0, &[int(v)]);
                    }
                    store.flush_hits(&mut c);
                    out
                })
            })
            .collect();
        let all: Vec<Vec<Handle>> = threads.into_iter().filter_map(|t| t.join().ok()).collect();
        assert_eq!(all.len(), 8);
        for (i, h) in (0..).zip(&all[0]) {
            assert_eq!(store.view(*h).data, &[int(i)]);
        }
        assert!(all.iter().all(|o| *o == all[0]));
        let st = store.stats();
        assert_eq!(st.nodes, 20_011);
        assert_eq!(st.hits + st.misses, 8 * 20_011);
        let memory = store.memory_usage();
        assert!(memory.current_table_bytes > 0);
        assert_eq!(
            memory.retired_tables, 0,
            "all worker caches have been dropped"
        );
        assert_eq!(memory.retired_table_bytes, 0);
        assert!(!store.poisoned());
    }

    #[test]
    fn without_hashconsing_every_intern_inserts() {
        let store = Store::new(false);
        let mut c = InternCache::default();
        let a = store.intern(&mut c, Kind::Set, 0, 0, 9, 9, &[int(1)]);
        let b = store.intern(&mut c, Kind::Set, 0, 0, 9, 9, &[int(1)]);
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
