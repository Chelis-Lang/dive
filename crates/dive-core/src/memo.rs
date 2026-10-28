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

use crate::handle::{Handle, mix2, mix64};

/// The largest key, in handles.
pub const MAX_KEY: usize = 8;
const WAYS_LOG: u32 = 1;
const SETS_LOG: u32 = 13;
const MIN_SETS_LOG: u32 = 6;
/// The default byte cap of one worker's tables.
pub const DEFAULT_CAP: usize = 64 << 20;
const PROBATION: u64 = 4096;
const MIN_HIT_PERCENT: u64 = 30;

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
        }
    }

    /// Bounds the bytes this worker's tables may hold. A table that would
    /// exceed it stops growing, and one that cannot be allocated at all is
    /// left empty (every lookup misses).
    pub fn set_cap(&mut self, bytes: usize) {
        self.cap = bytes;
    }

    /// Bytes held by the tables.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Whether the site is usable.
    #[must_use]
    pub fn active(&self, site: u32) -> bool {
        self.tables
            .get(site as usize)
            .and_then(Option::as_ref)
            .is_some_and(|t| !t.stats.off)
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
            if self.bytes + bytes_for(t.sets_log) > self.cap {
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
            && self.bytes - t.bytes() + grown <= self.cap
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
}
