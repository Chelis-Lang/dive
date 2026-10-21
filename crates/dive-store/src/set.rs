//! The seen-state set and its level-synchronous, deterministic resolve
//! (ADR 0015 §2, phase 2).
//!
//! # Protocol
//!
//! A level of the search uses a [`StateSet`] in two phases, separated by barriers:
//!
//! 1. **Expand (shared, read-only).** Workers hold `&S`. For each in-model successor
//!    they call [`StateSet::contains`]; a key already present is only counted. A key
//!    that may be new becomes a candidate, pushed with its **position** (its place in
//!    the reference order 𝒪) and any payload into the worker's own [`Candidates`]
//!    buffer, which sorts it into the right shard.
//! 2. **Resolve (exclusive).** The coordinator calls [`StateSet::resolve`] with every
//!    worker's buffer. Each shard is owned by exactly one thread for the phase (shards
//!    are handed out as `&mut` borrows; no atomics or locks guard the tables). A
//!    shard's candidates are sorted by key and position, the earliest position of each
//!    key wins, and the winner is inserted unless the key was already present.
//!
//! The winners are exactly the new states, each with the earliest position at which
//! it occurs. That is a function of the candidates alone: which worker produced which
//! candidate, how many threads resolve, and in what order shards are processed cannot
//! change it. The property tests check this under randomized schedules.
//!
//! Sequential code (initial states, small levels run inline) may instead call
//! [`StateSet::insert`] in position order, which gives the same set and the same
//! winners.

use std::sync::Mutex;

use crate::error::StoreError;

/// How a set decides that two states are the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreMode {
    /// 64-bit fingerprints: two distinct states may collide, and results carry TLC's
    /// collision-probability estimates ([`crate::collision`]).
    Fingerprint,
    /// Full canonical state encodings: exhaustiveness is unconditional (charter
    /// soundness rule 4).
    Exact,
}

impl StoreMode {
    /// The name used in output (`store: "fingerprint"` or `store: "exact"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fingerprint => "fingerprint",
            Self::Exact => "exact",
        }
    }
}

/// A set of seen states, keyed by fingerprint ([`crate::FpSet`]) or by canonical
/// encoding ([`crate::ExactSet`]).
pub trait StateSet: Send + Sync {
    /// A state's key: its fingerprint, or its canonical encoding.
    type Key: Ord + Send + Sync;

    /// How this set compares states.
    fn mode(&self) -> StoreMode;

    /// The 64-bit hash that places `key` in a shard and a slot. Equal keys have equal
    /// hashes.
    fn hash(&self, key: &Self::Key) -> u64;

    /// The number of shards (a power of two).
    fn shard_count(&self) -> usize;

    /// Whether `key` is in the set. Safe to call from many threads at once.
    fn contains(&self, key: &Self::Key) -> bool;

    /// Inserts `key`, returning whether it was new.
    fn insert(&mut self, key: Self::Key) -> bool;

    /// The number of states in the set.
    fn len(&self) -> u64;

    /// Whether the set is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The heap memory the set holds, in bytes.
    fn memory_bytes(&self) -> usize;

    /// An empty candidate buffer for this set, for one worker.
    fn candidates<P, T>(&self) -> Candidates<Self::Key, P, T> {
        Candidates::new(self.shard_count())
    }

    /// Resolves one level's candidates (phase 2): inserts the new keys and returns
    /// the winners, each the candidate with the earliest position for its key.
    ///
    /// # Errors
    /// [`StoreError::DuplicatePosition`] if two candidates share a key and a position,
    /// and [`StoreError::ShardMismatch`] for a buffer made for another set (caller
    /// bugs).
    fn resolve<P, T>(
        &mut self,
        batches: Vec<Candidates<Self::Key, P, T>>,
        opts: &ResolveOptions,
    ) -> Result<Resolved<Self::Key, P, T>, StoreError>
    where
        P: Ord + Copy + Send,
        T: Send;
}

/// A candidate new state: its key, its position in the reference order, and a
/// payload (for example the encoded state and action label) that travels with it.
#[derive(Clone, Debug)]
pub struct Candidate<K, P, T> {
    hash: u64,
    /// The state's key.
    pub key: K,
    /// The state's position in the level (ADR 0015 §1). Unique per candidate.
    pub pos: P,
    /// Data carried to the winner.
    pub payload: T,
}

impl<K, P, T> Candidate<K, P, T> {
    /// The key's hash (see [`StateSet::hash`]).
    #[must_use]
    pub const fn hash(&self) -> u64 {
        self.hash
    }
}

/// One worker's candidates for one level, already split by shard.
#[derive(Debug)]
pub struct Candidates<K, P, T> {
    per_shard: Vec<Vec<Candidate<K, P, T>>>,
    shard_bits: u32,
    len: usize,
}

impl<K, P, T> Candidates<K, P, T> {
    /// An empty buffer for a set with `shard_count` shards (a power of two). Prefer
    /// [`StateSet::candidates`].
    #[must_use]
    pub fn new(shard_count: usize) -> Self {
        let shard_count = shard_count.max(1).next_power_of_two();
        Self {
            per_shard: (0..shard_count).map(|_| Vec::new()).collect(),
            shard_bits: shard_count.trailing_zeros(),
            len: 0,
        }
    }

    /// Adds a candidate for `set`.
    pub fn push<S: StateSet<Key = K> + ?Sized>(&mut self, set: &S, key: K, pos: P, payload: T) {
        let hash = set.hash(&key);
        let shard = shard_of(hash, self.shard_bits);
        if let Some(v) = self.per_shard.get_mut(shard) {
            v.push(Candidate {
                hash,
                key,
                pos,
                payload,
            });
            self.len += 1;
        }
    }

    /// Moves matching positions into a same-shard buffer without hashing or
    /// comparing any stored key. Nonmatching candidates remain in original
    /// shard order, with their original hash and payload.
    #[must_use]
    pub fn extract_if(&mut self, mut selected: impl FnMut(&P) -> bool) -> Self {
        let mut result = Self::new(self.per_shard.len());
        for (shard, candidates) in self.per_shard.iter_mut().enumerate() {
            let mut retained = Vec::with_capacity(candidates.len());
            for candidate in std::mem::take(candidates) {
                if selected(&candidate.pos) {
                    result.per_shard[shard].push(candidate);
                    result.len += 1;
                } else {
                    retained.push(candidate);
                }
            }
            *candidates = retained;
        }
        self.len -= result.len;
        result
    }

    /// The number of candidates.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no candidates.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of shards this buffer was built for.
    #[must_use]
    pub fn shard_count(&self) -> usize {
        self.per_shard.len()
    }

    /// Consumes buffered candidates in reference position order, retaining keys
    /// and payloads without inserting any into a seen set. Coordinators can use
    /// this to enforce an explicit admission ceiling before publication.
    #[must_use]
    pub fn into_position_order(self) -> Vec<Candidate<K, P, T>>
    where
        P: Ord,
    {
        let mut all: Vec<_> = self.per_shard.into_iter().flatten().collect();
        all.sort_unstable_by(|a, b| a.pos.cmp(&b.pos));
        all
    }
}

/// The shard of `hash` among `2^bits` shards: its top `bits` bits.
pub(crate) fn shard_of(hash: u64, bits: u32) -> usize {
    if bits == 0 {
        0
    } else {
        usize::try_from(hash >> (64 - bits)).unwrap_or(0)
    }
}

/// How to run a resolve. Results never depend on these settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolveOptions {
    /// Threads to use (at least one).
    pub threads: usize,
    /// Test-only chaos: when set, shards are claimed in an order shuffled by this
    /// seed and threads yield at random points. Results must not change.
    pub chaos: Option<u64>,
}

impl ResolveOptions {
    /// `threads` threads, no chaos.
    #[must_use]
    pub const fn threads(threads: usize) -> Self {
        Self {
            threads,
            chaos: None,
        }
    }
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self::threads(1)
    }
}

/// The winners of a resolve, grouped by shard and sorted by key within a shard.
#[derive(Debug)]
pub struct Resolved<K, P, T> {
    per_shard: Vec<Vec<Candidate<K, P, T>>>,
}

impl<K, P: Ord + Copy, T> Resolved<K, P, T> {
    /// The number of winners: the level's new states.
    #[must_use]
    pub fn len(&self) -> usize {
        self.per_shard.iter().map(Vec::len).sum()
    }

    /// Whether there were no new states.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.per_shard.iter().all(Vec::is_empty)
    }

    /// The winners in shard order, then key order.
    pub fn iter(&self) -> impl Iterator<Item = &Candidate<K, P, T>> {
        self.per_shard.iter().flatten()
    }

    /// The winners of one shard, sorted by key.
    #[must_use]
    pub fn shard(&self, shard: usize) -> &[Candidate<K, P, T>] {
        self.per_shard.get(shard).map_or(&[], Vec::as_slice)
    }

    /// The winners in position order: the next level's frontier order.
    #[must_use]
    pub fn into_position_order(self) -> Vec<Candidate<K, P, T>> {
        let mut all: Vec<_> = self.per_shard.into_iter().flatten().collect();
        all.sort_unstable_by_key(|c| c.pos);
        all
    }
}

/// One shard's table, as the resolve engine sees it.
pub(crate) trait Shard: Send {
    type Key: Ord + Send;
    /// Inserts `key`, returning whether it was new.
    fn insert_hashed(&mut self, hash: u64, key: &Self::Key) -> bool;
}

/// A deterministic, seedable generator for chaos schedules (`splitmix64`).
struct Chaos(u64);

impl Chaos {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn maybe_yield(this: &mut Option<Self>) {
        if let Some(c) = this
            && c.next() % 8 == 0
        {
            std::thread::yield_now();
        }
    }

    fn shuffle<X>(&mut self, v: &mut [X]) {
        for i in (1..v.len()).rev() {
            let j = usize::try_from(self.next() % (i as u64 + 1)).unwrap_or(0);
            v.swap(i, j);
        }
    }
}

type Task<'s, S, P, T> = (usize, &'s mut S, Vec<Candidate<<S as Shard>::Key, P, T>>);
type Outcome<K, P, T> = (usize, Result<Vec<Candidate<K, P, T>>, StoreError>);

/// Phase 2 over `shards` (see the module documentation).
pub(crate) fn resolve_shards<S, P, T>(
    shards: &mut [S],
    batches: Vec<Candidates<S::Key, P, T>>,
    opts: &ResolveOptions,
) -> Result<Resolved<S::Key, P, T>, StoreError>
where
    S: Shard,
    P: Ord + Copy + Send,
    T: Send,
{
    let n = shards.len();
    // Gather each shard's candidates from every worker's buffer.
    let mut gathered: Vec<Vec<Candidate<S::Key, P, T>>> = (0..n).map(|_| Vec::new()).collect();
    for batch in batches {
        if batch.per_shard.len() != n {
            return Err(StoreError::ShardMismatch {
                expected: n,
                found: batch.per_shard.len(),
            });
        }
        for (g, mut v) in gathered.iter_mut().zip(batch.per_shard) {
            if g.is_empty() {
                *g = v;
            } else {
                g.append(&mut v);
            }
        }
    }
    let mut tasks: Vec<Task<'_, S, P, T>> = shards
        .iter_mut()
        .zip(gathered)
        .enumerate()
        .filter(|(_, (_, c))| !c.is_empty())
        .map(|(i, (s, c))| (i, s, c))
        .collect();
    let mut chaos = opts.chaos.map(Chaos);
    if let Some(c) = &mut chaos {
        c.shuffle(&mut tasks);
    }
    let threads = opts.threads.max(1).min(tasks.len().max(1));
    let mut outcomes: Vec<Outcome<S::Key, P, T>> = if threads == 1 {
        tasks
            .into_iter()
            .map(|(i, s, c)| (i, resolve_one(i, s, c, &mut chaos)))
            .collect()
    } else {
        // Threads claim shards from a shared queue: each shard is resolved by exactly
        // one thread, through the `&mut` borrow it claimed.
        tasks.reverse();
        let queue = Mutex::new(tasks);
        let queue = &queue;
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let seed = opts
                        .chaos
                        .map(|c| c ^ (t as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
                    scope.spawn(move || {
                        let mut chaos = seed.map(Chaos);
                        let mut done = Vec::new();
                        loop {
                            Chaos::maybe_yield(&mut chaos);
                            let task = queue
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .pop();
                            let Some((i, s, c)) = task else { break };
                            done.push((i, resolve_one(i, s, c, &mut chaos)));
                        }
                        done
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
                .collect()
        })
    };
    outcomes.sort_unstable_by_key(|(i, _)| *i);
    let mut per_shard: Vec<Vec<Candidate<S::Key, P, T>>> = (0..n).map(|_| Vec::new()).collect();
    for (i, r) in outcomes {
        if let Some(slot) = per_shard.get_mut(i) {
            *slot = r?;
        }
    }
    Ok(Resolved { per_shard })
}

/// Resolves one shard: earliest position per key, keys already present excluded.
fn resolve_one<S, P, T>(
    shard_index: usize,
    shard: &mut S,
    mut cands: Vec<Candidate<S::Key, P, T>>,
    chaos: &mut Option<Chaos>,
) -> Result<Vec<Candidate<S::Key, P, T>>, StoreError>
where
    S: Shard,
    P: Ord + Copy,
{
    // Unstable sorting is deterministic here: (hash, key, position) is unique, and a
    // duplicate is reported below.
    cands.sort_unstable_by(|a, b| {
        a.hash
            .cmp(&b.hash)
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| a.pos.cmp(&b.pos))
    });
    let mut winners = Vec::new();
    let mut iter = cands.into_iter().peekable();
    while let Some(first) = iter.next() {
        Chaos::maybe_yield(chaos);
        while let Some(next) = iter.peek() {
            if next.hash != first.hash || next.key != first.key {
                break;
            }
            if next.pos == first.pos {
                return Err(StoreError::DuplicatePosition { shard: shard_index });
            }
            iter.next();
        }
        if shard.insert_hashed(first.hash, &first.key) {
            winners.push(first);
        }
    }
    Ok(winners)
}

/// The number of shard bits for a run with `workers` workers: about eight shards per
/// worker, so that resolve load-balances, and at most 4096 shards.
#[must_use]
pub fn shard_bits_for(workers: usize) -> u32 {
    let shards = workers.max(1).saturating_mul(8).next_power_of_two();
    shards.trailing_zeros().min(12)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "forced collision admission assertions")]
mod prefix_collision_tests {
    use super::{Candidates, ResolveOptions, StateSet};
    use crate::ExactSet;

    #[test]
    fn partition_preserves_forced_collision_hashes_and_original_exact_keys() {
        // Deliberately supply one stored hash for every distinct exact key.
        // Never recompute it during partition; normal key hashing is irrelevant
        // to this adversarial internal resolve control.
        let mut set = ExactSet::new(1);
        let mut buffer: Candidates<Box<[u8]>, usize, usize> = set.candidates();
        for (key, pos) in [(b"a", 3), (b"b", 2), (b"a", 0), (b"b", 1), (b"c", 4)] {
            buffer.push(&set, key.to_vec().into_boxed_slice(), pos, pos);
        }
        for candidate in &mut buffer.per_shard[0] {
            candidate.hash = 0;
        }
        let first = buffer.extract_if(|pos| *pos < 2);
        assert!(first.per_shard[0].iter().all(|c| c.hash == 0));
        assert!(buffer.per_shard[0].iter().all(|c| c.hash == 0));
        let first = set
            .resolve(vec![first], &ResolveOptions::threads(1))
            .expect("prefix collision resolve")
            .into_position_order();
        assert_eq!(
            first
                .iter()
                .map(|c| (c.key.as_ref(), c.pos, c.payload))
                .collect::<Vec<_>>(),
            vec![(b"a".as_slice(), 0, 0), (b"b".as_slice(), 1, 1)]
        );
        let later = set
            .resolve(vec![buffer], &ResolveOptions::threads(4))
            .expect("retained collision resolve")
            .into_position_order();
        assert_eq!(
            later
                .iter()
                .map(|c| (c.key.as_ref(), c.pos, c.payload))
                .collect::<Vec<_>>(),
            vec![(b"c".as_slice(), 4, 4)]
        );
        assert_eq!(set.len(), 3);
    }
}
