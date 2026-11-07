//! Keyed result caches for pure bodies (`06-memoization.md` §2.2).
//!
//! Each site has a per-worker, two-way set-associative table. An entry holds
//! the full key (the handles of the site's inputs) and the result. A site
//! whose hit rate is under 30% after 4,096 lookups turns itself off.
//!
//! A table starts small and doubles when it evicts more entries than it
//! holds, up to 2^13 sets, while the worker's tables stay within their byte
//! cap ([`Memo::set_cap`]). Eviction only drops a cached result, which is
//! recomputed on the next lookup, so neither the cap nor the eviction
//! changes a result.

use std::sync::{Arc, Weak};

use crate::handle::{Handle, mix2, mix64};
use crate::store::Store;

/// The largest key, in handles.
pub const MAX_KEY: usize = 8;
const WAYS_LOG: u32 = 1;
const SETS_LOG: u32 = 13;
const MIN_SETS_LOG: u32 = 6;
/// The default byte cap of one worker's tables.
pub const DEFAULT_CAP: usize = 64 << 20;
const PROBATION: u64 = 4096;
const MIN_HIT_PERCENT: u64 = 30;
const MEMBER_CAP: usize = 64 << 10;

/// A bounded, worker-local certificate that one persistent member has the
/// exact aggregate comparison shape of its persistent set. Keys and witnesses
/// are complete handles; the hash selects a set, never establishes a hit.
#[derive(Debug, Default)]
struct Members {
    entries: Vec<[Handle; 2]>,
    cap: usize,
    flip: bool,
    owner: Option<Weak<Store>>,
}

impl Members {
    fn bind(&mut self, store: &Arc<Store>) {
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| std::ptr::eq(owner.as_ptr(), Arc::as_ptr(store)))
        {
            return;
        }
        // Weak retains the allocation identity even after Store destruction,
        // preventing pointer ABA without retaining the store's contents. Hits
        // only compare pointers: no upgrade or reference-count operation.
        self.clear();
        self.owner = Some(Arc::downgrade(store));
    }

    fn clear(&mut self) {
        self.entries = Vec::new();
        self.owner = None;
    }

    fn bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<[Handle; 2]>()
    }

    fn base(set: Handle, entries: usize) -> usize {
        #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
        let slot = (mix64(set.bits()) as usize) & (entries / 2 - 1);
        slot * 2
    }

    fn get(&self, set: Handle) -> Option<Handle> {
        if self.entries.is_empty() {
            return None;
        }
        let base = Self::base(set, self.entries.len());
        self.entries[base..base + 2]
            .iter()
            .find(|entry| entry[0] == set)
            .map(|entry| entry[1])
    }

    fn put(&mut self, set: Handle, witness: Handle) {
        if self.cap == 0 || !set.is_persistent() || !witness.is_persistent() {
            return;
        }
        if self.entries.is_empty() {
            self.entries = vec![[Handle::EMPTY; 2]; self.cap / 16];
        }
        let base = Self::base(set, self.entries.len());
        let way = self.entries[base..base + 2]
            .iter()
            .position(|entry| entry[0] == set || entry[0] == Handle::EMPTY)
            .unwrap_or_else(|| {
                self.flip = !self.flip;
                usize::from(self.flip)
            });
        self.entries[base + way] = [set, witness];
    }
}

/// Per-site statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiteStats {
    /// Lookups made.
    pub lookups: u64,
    /// Lookups answered from the table.
    pub hits: u64,
    /// Evaluations that bypassed the table (a transient input).
    pub bypass: u64,
    /// Whether the site turned itself off.
    pub off: bool,
}

#[derive(Debug)]
struct Table {
    klen: usize,
    /// `sets * ways` entries of `1 + klen` words: the result, then the key.
    words: Vec<u64>,
    stats: SiteStats,
    flip: bool,
    /// The table has `1 << sets_log` sets once allocated.
    sets_log: u32,
    /// Evictions since the table last grew.
    evictions: usize,
}

impl Table {
    fn bytes(&self) -> usize {
        self.words.len() * 8
    }
}

/// The memo tables of one worker.
#[derive(Debug, Default)]
pub struct Memo {
    tables: Vec<Option<Table>>,
    /// Bytes held by the tables.
    bytes: usize,
    /// The most the tables may hold, in bytes.
    cap: usize,
    /// About the bytes of the persistent copies that candidate sites
    /// (`MemoSite::candidate`) have added to the store: at most `cap`
    /// before they all turn off ([`Memo::charge_kept`]).
    kept: usize,
    members: Members,
    pub(crate) member_index: crate::member_index::MemberIndex,
}

impl Memo {
    /// Tables for sites with these key lengths.
    #[must_use]
    pub fn new(key_lens: &[usize]) -> Self {
        Self {
            tables: key_lens
                .iter()
                .map(|&klen| {
                    (klen <= MAX_KEY).then(|| Table {
                        klen,
                        words: Vec::new(),
                        stats: SiteStats::default(),
                        flip: false,
                        sets_log: MIN_SETS_LOG,
                        evictions: 0,
                    })
                })
                .collect(),
            bytes: 0,
            cap: DEFAULT_CAP,
            kept: 0,
            members: Members::default(),
            member_index: crate::member_index::MemberIndex::default(),
        }
    }

    /// Bounds the bytes this worker's tables may hold. A table that would
    /// exceed it stops growing, and one that cannot be allocated at all is
    /// left empty (every lookup misses).
    pub fn set_cap(&mut self, bytes: usize) {
        let indexed = self.member_index.cap != 0;
        self.cap = bytes;
        self.member_index.clear();
        self.member_index.cap = 0;
        if self.members_active() {
            // Only the new certificate storage is discarded/reconfigured.
            // Existing body-table allocations retain their prior contract.
            self.reserve_members(true);
        }
        self.reserve_member_index(indexed);
    }

    /// Reserves part of the existing cap for exact membership certificates.
    /// No new allowance is added; disabled/default-profile machines reserve
    /// nothing. Allocation stays lazy, and the size is a power of two for
    /// two-way indexing. Machine construction and public cap changes
    /// configure this reservation.
    pub(crate) fn reserve_members(&mut self, enabled: bool) {
        // Machine construction must not reuse certificates from another
        // store, whose handles may have the same bits.
        self.members.clear();
        let bytes = if enabled {
            (self.cap / 16)
                .min(MEMBER_CAP)
                .min(self.cap.saturating_sub(self.bytes + self.member_index.cap))
        } else {
            0
        };
        self.members.cap = if bytes < 32 { 0 } else { 1 << bytes.ilog2() };
    }

    /// Reserves index storage inside, never in addition to, the existing cap.
    pub(crate) fn reserve_member_index(&mut self, enabled: bool) {
        self.member_index.clear();
        self.member_index.cap = if enabled {
            (self.cap / 16)
                .min(4 << 20)
                .min(self.cap.saturating_sub(self.bytes + self.members.cap))
        } else {
            0
        };
    }

    pub(crate) fn member_witness(&mut self, store: &Arc<Store>, set: Handle) -> Option<Handle> {
        if !self.members_active() {
            return None;
        }
        self.members.bind(store);
        self.members.get(set)
    }

    pub(crate) fn remember_member(&mut self, store: &Arc<Store>, set: Handle, witness: Handle) {
        if !self.members_active() || self.members.cap > self.cap.saturating_sub(self.bytes) {
            return;
        }
        self.members.bind(store);
        self.members.put(set, witness);
    }

    pub(crate) fn members_active(&self) -> bool {
        self.members.cap != 0
    }

    /// Bytes held by the tables.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes + self.members.bytes() + self.member_index.bytes()
    }

    /// Whether the site is usable.
    #[must_use]
    pub fn active(&self, site: u32) -> bool {
        self.tables
            .get(site as usize)
            .and_then(Option::as_ref)
            .is_some_and(|t| !t.stats.off)
    }

    /// Whether candidate sites may still add persistent copies to the
    /// store: until they have added the tables' byte cap.
    #[must_use]
    pub fn may_keep(&self) -> bool {
        self.kept <= self.cap
    }

    /// Charges the bytes a candidate site's keys and result added to the
    /// store. The store never frees them, so once candidate sites have
    /// added the tables' byte cap in all, the site turns off, and so does
    /// every other candidate site at its next lookup ([`Memo::may_keep`]).
    /// This bounds what keys that are values of enumeration candidates
    /// can retain; turning a site off only drops cached results.
    pub fn charge_kept(&mut self, site: u32, bytes: u64) {
        self.kept = self
            .kept
            .saturating_add(usize::try_from(bytes).unwrap_or(usize::MAX));
        if self.may_keep() {
            return;
        }
        if let Some(Some(t)) = self.tables.get_mut(site as usize) {
            t.stats.off = true;
            self.bytes -= t.bytes();
            t.words = Vec::new();
        }
    }

    /// Records an evaluation that bypassed the table.
    pub fn bypass(&mut self, site: u32) {
        if let Some(Some(t)) = self.tables.get_mut(site as usize) {
            t.stats.bypass += 1;
        }
    }

    fn slot_of(key: &[Handle], sets_log: u32) -> usize {
        let mut h = mix64(key.len() as u64);
        for k in key {
            h = mix2(h, k.bits());
        }
        #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
        let s = (h as usize) & ((1 << sets_log) - 1);
        s << WAYS_LOG
    }

    /// Places an entry (`1 + klen` words) in a table of `sets_log` sets,
    /// evicting a way of a full set. Returns whether it evicted.
    fn place(words: &mut [u64], flip: &mut bool, sets_log: u32, entry: &[u64]) -> bool {
        let stride = entry.len();
        let key: Vec<Handle> = entry[1..].iter().map(|&b| Handle(b)).collect();
        let base = Self::slot_of(&key, sets_log);
        let free =
            (0..(1 << WAYS_LOG)).find(|w| words[(base + w) * stride] == Handle::EMPTY.bits());
        let way = free.unwrap_or_else(|| {
            *flip = !*flip;
            usize::from(*flip)
        });
        let at = (base + way) * stride;
        words[at..at + stride].copy_from_slice(entry);
        free.is_none()
    }

    /// Looks a key up.
    pub fn get(&mut self, site: u32, key: &[Handle]) -> Option<Handle> {
        let t = self.tables.get_mut(site as usize)?.as_mut()?;
        t.stats.lookups += 1;
        if t.words.is_empty() {
            return None;
        }
        let stride = 1 + t.klen;
        let base = Self::slot_of(key, t.sets_log);
        for way in 0..(1 << WAYS_LOG) {
            let at = (base + way) * stride;
            let w = &t.words[at..at + stride];
            if w[0] != Handle::EMPTY.bits() && w[1..].iter().zip(key).all(|(a, b)| *a == b.bits()) {
                t.stats.hits += 1;
                return Some(Handle(w[0]));
            }
        }
        if t.stats.lookups >= PROBATION && t.stats.hits * 100 < t.stats.lookups * MIN_HIT_PERCENT {
            t.stats.off = true;
            self.bytes -= t.bytes();
            t.words = Vec::new();
        }
        None
    }

    /// Records a result.
    pub fn put(&mut self, site: u32, key: &[Handle], result: Handle) {
        let Some(Some(t)) = self.tables.get_mut(site as usize) else {
            return;
        };
        if t.stats.off {
            return;
        }
        let stride = 1 + t.klen;
        let bytes_for = |sets_log: u32| (1usize << (sets_log + WAYS_LOG)) * stride * 8;
        if t.words.is_empty() {
            if self.bytes + bytes_for(t.sets_log)
                > self
                    .cap
                    .saturating_sub(self.members.cap + self.member_index.cap)
            {
                return;
            }
            t.words = vec![Handle::EMPTY.bits(); bytes_for(t.sets_log) / 8];
            self.bytes += t.bytes();
        }
        let mut entry = [0u64; 1 + MAX_KEY];
        entry[0] = result.bits();
        for (i, k) in key.iter().enumerate() {
            entry[1 + i] = k.bits();
        }
        let entry = &entry[..stride];
        if Self::place(&mut t.words, &mut t.flip, t.sets_log, entry) {
            t.evictions += 1;
        }
        let grown = bytes_for(t.sets_log + 1);
        if t.evictions > 1 << (t.sets_log + WAYS_LOG - 2)
            && t.sets_log < SETS_LOG
            && self.bytes - t.bytes() + grown
                <= self
                    .cap
                    .saturating_sub(self.members.cap + self.member_index.cap)
        {
            // Rehash every entry into a table twice the size.
            let mut words = vec![Handle::EMPTY.bits(); grown / 8];
            for old in t.words.chunks_exact(stride) {
                if old[0] != Handle::EMPTY.bits() {
                    Self::place(&mut words, &mut t.flip, t.sets_log + 1, old);
                }
            }
            self.bytes = self.bytes - t.bytes() + grown;
            t.words = words;
            t.sets_log += 1;
            t.evictions = 0;
        }
    }

    /// Statistics per site.
    #[must_use]
    pub fn stats(&self) -> Vec<SiteStats> {
        self.tables
            .iter()
            .map(|t| t.as_ref().map(|t| t.stats).unwrap_or_default())
            .collect()
    }

    /// Clears every table (store compaction remaps handles).
    pub fn clear(&mut self) {
        for t in self.tables.iter_mut().flatten() {
            t.words = Vec::new();
        }
        self.bytes = 0;
        self.members.clear();
        self.member_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_then_get_and_key_mismatch() {
        let mut m = Memo::new(&[2]);
        let k1 = [Handle::int(1).unwrap_or(Handle::EMPTY), Handle::TRUE];
        let k2 = [Handle::int(2).unwrap_or(Handle::EMPTY), Handle::TRUE];
        assert_eq!(m.get(0, &k1), None);
        m.put(0, &k1, Handle::FALSE);
        assert_eq!(m.get(0, &k1), Some(Handle::FALSE));
        assert_eq!(m.get(0, &k2), None);
        let s = m.stats()[0];
        assert_eq!((s.lookups, s.hits), (3, 1));
    }

    #[test]
    fn candidate_sites_turn_off_once_their_copies_reach_the_cap() {
        let mut m = Memo::new(&[1, 1]);
        m.set_cap(4096);
        let k = [Handle::TRUE];
        m.put(0, &k, Handle::FALSE);
        m.charge_kept(0, 4096);
        assert!(m.may_keep() && m.active(0));
        assert_eq!(m.get(0, &k), Some(Handle::FALSE));
        m.charge_kept(0, 1);
        assert!(!m.may_keep());
        assert!(!m.active(0) && m.stats()[0].off);
        assert_eq!(m.get(0, &k), None);
        // Another site is not dropped, but its caller sees `may_keep` false.
        assert!(m.active(1));
    }

    #[test]
    fn a_cold_site_turns_itself_off() {
        let mut m = Memo::new(&[1]);
        for i in 0..5000 {
            let k = [Handle::int(i).unwrap_or(Handle::EMPTY)];
            if m.get(0, &k).is_none() {
                m.put(0, &k, Handle::TRUE);
            }
        }
        assert!(m.stats()[0].off);
        assert!(!m.active(0));
        assert_eq!(m.bytes(), 0);
    }

    #[test]
    fn tables_grow_within_the_cap_and_never_answer_wrongly() {
        let key = |i: i64| [Handle::int(i).unwrap_or(Handle::EMPTY)];
        let val = |i: i64| Handle::int(i * 7).unwrap_or(Handle::EMPTY);
        for cap in [0, 4096, 64 << 10, DEFAULT_CAP] {
            let mut m = Memo::new(&[1]);
            m.set_cap(cap);
            for round in 0..3 {
                for i in 0..20_000 {
                    match m.get(0, &key(i)) {
                        Some(v) => assert_eq!(v, val(i), "cap {cap} round {round}"),
                        None => m.put(0, &key(i), val(i)),
                    }
                }
            }
            assert!(m.bytes() <= cap, "cap {cap}: {} bytes", m.bytes());
        }
    }

    #[test]
    fn member_certificates_are_exact_bounded_and_cleared() {
        let persistent = |i| Handle::persistent(0, 0, i);
        for cap in [0, 31, 512, 4096, 1 << 20, DEFAULT_CAP] {
            let mut m = Memo::new(&[1]);
            let store = Arc::new(Store::default());
            m.set_cap(cap);
            m.reserve_members(true);
            assert!(m.members.cap <= cap / 16);
            assert!(m.members.cap <= MEMBER_CAP);
            assert_eq!(m.bytes(), 0, "allocation is lazy");
            for i in 1..20_000 {
                let set = persistent(i);
                let witness = persistent(i + 20_000);
                if let Some(found) = m.member_witness(&store, set) {
                    assert_eq!(found, witness, "a hash collision is not a hit");
                }
                m.remember_member(&store, set, witness);
                if m.members_active() {
                    assert_eq!(m.member_witness(&store, set), Some(witness));
                }
                m.put(0, &[set], witness);
                assert!(m.bytes() <= cap);
                assert_eq!(m.bytes(), m.bytes + m.members.bytes());
            }
            assert!(m.members.entries.len() <= m.members.cap / 16);
            m.clear();
            assert_eq!(m.bytes(), 0);
            assert_eq!(m.member_witness(&store, persistent(1)), None);
        }
    }

    #[test]
    fn member_certificates_do_not_reserve_when_disabled_or_store_transients() {
        let store = Arc::new(Store::default());
        let mut m = Memo::new(&[1]);
        m.reserve_members(false);
        assert!(!m.members_active());
        assert_eq!(m.cap - m.members.cap, DEFAULT_CAP);
        m.reserve_members(true);
        let persistent = Handle::persistent(0, 0, 1);
        let transient = Handle::transient(1, 1);
        m.remember_member(&store, transient, persistent);
        m.remember_member(&store, persistent, transient);
        assert_eq!(m.bytes(), 0);
        assert_eq!(m.member_witness(&store, persistent), None);
    }

    #[test]
    fn member_collision_eviction_never_answers_for_a_different_key() {
        let store = Arc::new(Store::default());
        let mut m = Memo::new(&[]);
        m.set_cap(512);
        m.reserve_members(true);
        // A 32-byte reservation has exactly one two-way bucket. All keys
        // below deliberately collide, independently of the hash function.
        let a = Handle::persistent(0, 0, 1);
        let b = Handle::persistent(0, 0, 2);
        let c = Handle::persistent(0, 0, 3);
        m.remember_member(&store, a, a);
        m.remember_member(&store, b, b);
        assert_eq!(m.member_witness(&store, c), None);
        assert_eq!(m.member_witness(&store, a), Some(a));
        assert_eq!(m.member_witness(&store, b), Some(b));
        m.remember_member(&store, c, c);
        assert_eq!(m.member_witness(&store, c), Some(c));
        for key in [a, b] {
            if let Some(witness) = m.member_witness(&store, key) {
                assert_eq!(witness, key);
            }
        }
        assert_eq!(m.bytes(), 32);
    }

    #[test]
    fn member_owner_binding_releases_entries_and_weak_ownership() {
        let mut m = Memo::new(&[]);
        m.reserve_members(true);
        let store = Arc::new(Store::default());
        let other = Arc::new(Store::default());
        let set = Handle::persistent(0, 0, 1);
        let witness = Handle::persistent(0, 0, 2);
        m.remember_member(&store, set, witness);
        assert_eq!(Arc::weak_count(&store), 1);
        for _ in 0..100 {
            assert_eq!(m.member_witness(&store, set), Some(witness));
        }
        assert_eq!(Arc::weak_count(&store), 1);
        assert_eq!(m.member_witness(&other, set), None);
        assert_eq!(m.bytes(), 0);
        assert_eq!(Arc::weak_count(&store), 0);
        assert_eq!(Arc::weak_count(&other), 1);
        m.remember_member(&other, set, witness);
        m.clear();
        assert_eq!(Arc::weak_count(&other), 0);
        m.remember_member(&store, set, witness);
        m.reserve_members(false);
        assert_eq!(Arc::weak_count(&store), 0);
        assert_eq!(m.member_witness(&store, set), None);
        assert_eq!(Arc::weak_count(&store), 0);

        m.reserve_members(true);
        m.remember_member(&store, set, witness);
        let identity = m.members.owner.as_ref().map(Weak::as_ptr);
        assert!(identity.is_some());
        drop(store);
        assert!(
            m.members
                .owner
                .as_ref()
                .is_some_and(|owner| owner.upgrade().is_none())
        );
        // The retained Weak keeps the old allocation unavailable for reuse.
        assert_ne!(identity, Some(Arc::as_ptr(&other)));
        assert_eq!(m.member_witness(&other, set), None);
        assert_eq!(m.bytes(), 0);
        m.clear();
        assert!(m.members.owner.is_none());
        assert_eq!(Arc::weak_count(&other), 0);
    }
}
