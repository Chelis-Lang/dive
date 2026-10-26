//! Keyed result caches for pure bodies (`06-memoization.md` §2.2).
//!
//! Each site has a per-worker, two-way set-associative table. An entry holds
//! the full key (the handles of the site's inputs) and the result. A site
//! whose hit rate is under 30% after 4,096 lookups turns itself off.

use crate::handle::{Handle, mix2, mix64};

/// The largest key, in handles.
pub const MAX_KEY: usize = 8;
const WAYS_LOG: u32 = 1;
const SETS_LOG: u32 = 13;
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
}

/// The memo tables of one worker.
#[derive(Debug, Default)]
pub struct Memo {
    tables: Vec<Option<Table>>,
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
                    })
                })
                .collect(),
        }
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

    fn slot_of(key: &[Handle]) -> usize {
        let mut h = mix64(key.len() as u64);
        for k in key {
            h = mix2(h, k.bits());
        }
        #[allow(clippy::cast_possible_truncation, reason = "masked to the table size")]
        let s = (h as usize) & ((1 << SETS_LOG) - 1);
        s << WAYS_LOG
    }

    /// Looks a key up.
    pub fn get(&mut self, site: u32, key: &[Handle]) -> Option<Handle> {
        let t = self.tables.get_mut(site as usize)?.as_mut()?;
        t.stats.lookups += 1;
        if t.words.is_empty() {
            return None;
        }
        let stride = 1 + t.klen;
        let base = Self::slot_of(key);
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
        if t.words.is_empty() {
            t.words = vec![Handle::EMPTY.bits(); (1 << (SETS_LOG + WAYS_LOG)) * stride];
        }
        let base = Self::slot_of(key);
        let mut way =
            (0..(1 << WAYS_LOG)).find(|w| t.words[(base + w) * stride] == Handle::EMPTY.bits());
        if way.is_none() {
            t.flip = !t.flip;
            way = Some(usize::from(t.flip));
        }
        let at = (base + way.unwrap_or(0)) * stride;
        t.words[at] = result.bits();
        for (i, k) in key.iter().enumerate() {
            t.words[at + 1 + i] = k.bits();
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
    }
}
