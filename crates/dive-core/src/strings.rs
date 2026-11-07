//! The global, append-only string table (`03-value-model.md` §2, §7).
//!
//! Ids are assigned on first encounter. The default profile orders strings by
//! their UTF-8 bytes ([`StringOrder::Text`]), so ids carry no order there.
//! Under `--compat=tlc` strings compare by intern token
//! ([`StringOrder::Token`]), which reproduces `UniqueString.compareTo`: the
//! id is the token, so the table must first be fed TLC's startup encounter
//! order with [`Strings::preload`] (`03-value-model.md` §7).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, PoisonError, RwLock};

/// How strings compare (`02-semantic-contract.md` D1, D2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StringOrder {
    /// By UTF-8 bytes: the default profile's canonical order (D-010).
    #[default]
    Text,
    /// By intern token, in encounter order: TLC's order.
    Token,
}

/// The string table.
#[derive(Debug, Default)]
pub struct Strings {
    inner: RwLock<Inner>,
    order: StringOrder,
    sealed: AtomicBool,
}

#[derive(Debug, Default)]
struct Inner {
    ids: HashMap<Arc<str>, u32>,
    texts: Vec<Arc<str>>,
    payload_bytes: usize,
    longest_bytes: usize,
    /// The dense rank of each id below `ranks.len()` in the table's order
    /// among those ids ([`Strings::ranks`]).
    ranks: Arc<[u32]>,
}

/// Up to this many strings, [`Strings::ranks`] ranks every new string at
/// once; past it, only once the unranked ids reach an eighth of the ranked
/// ones, so ranking a table that grows one string at a time costs a
/// constant factor over sorting it once.
const RANK_ALL_UP_TO: usize = 1024;

/// Whether [`Strings::ranks`] should re-rank `len` ids of which `ranked`
/// are ranked.
fn rank_stale(ranked: usize, len: usize) -> bool {
    let unranked = len.saturating_sub(ranked);
    unranked > 0 && (len <= RANK_ALL_UP_TO || unranked.saturating_mul(8) >= ranked)
}

/// Known storage and opaque index capacity of an append-only string table.
///
/// Text payload is counted once, even though the index, text vector and callers
/// share each string. Arc control blocks, allocator overhead and the physical
/// `HashMap` bucket allocation are not exposed by stable standard-library APIs.
/// The rank index ([`Strings::ranks`]), which heaps share, is not counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StringsMemory {
    /// Unique UTF-8 payload bytes, excluding Arc control blocks.
    pub payload_bytes: usize,
    /// Allocated text-vector slots, including unused capacity.
    pub text_slots_bytes: usize,
    /// Live index key/value object bytes, excluding padding and vacant buckets.
    pub index_entries_bytes: usize,
    /// Logical `HashMap` capacity; this is not a physical bucket-byte count.
    pub index_capacity: usize,
    /// Unique strings retained by the table.
    pub strings: usize,
}

impl StringsMemory {
    /// A lower bound on dynamic retained bytes, or `None` on arithmetic overflow.
    /// The containing [`Strings`] object itself is excluded.
    #[must_use]
    pub fn known_retained_bytes(self) -> Option<usize> {
        self.payload_bytes
            .checked_add(self.text_slots_bytes)?
            .checked_add(self.index_entries_bytes)
    }
}

impl Strings {
    /// An empty table with the given order.
    #[must_use]
    pub fn new(order: StringOrder) -> Self {
        Self {
            inner: RwLock::default(),
            order,
            sealed: AtomicBool::new(false),
        }
    }

    /// The order strings compare in.
    #[must_use]
    pub fn order(&self) -> StringOrder {
        self.order
    }

    /// Interns `texts` in the given order, so that under
    /// [`StringOrder::Token`] they compare in that order. Call it before
    /// anything else is interned: TLC's tokens are assigned in its startup
    /// encounter order (configuration, built-ins, lexer tokens, semantic
    /// names), and the reference binder records that order.
    ///
    /// # Errors
    /// `texts` repeats a string, or the table already holds strings that are
    /// not a prefix of `texts`, so the tokens would not be TLC's; or more
    /// than `u32::MAX` strings.
    pub fn preload<'a>(
        &self,
        texts: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), crate::Error> {
        for (n, t) in texts.into_iter().enumerate() {
            if self.intern(t)? as usize != n {
                return Err(crate::Error::internal(
                    "string tokens preloaded out of order: the table must be a prefix of distinct texts",
                ));
            }
        }
        Ok(())
    }

    /// Refuses every later new string: under [`StringOrder::Token`] a string
    /// TLC first meets during the search gets a token that depends on what
    /// TLC evaluated before it, so after [`Strings::preload`] the run seals
    /// the table rather than guess the order.
    pub fn seal(&self) {
        self.sealed.store(true, AtomicOrdering::Release);
    }

    /// Whether this table refuses every name absent from its current domain.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.sealed.load(AtomicOrdering::Acquire)
    }

    /// The id of `text` if the table already holds it, never assigning one.
    #[must_use]
    pub fn lookup(&self, text: &str) -> Option<u32> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .ids
            .get(text)
            .copied()
    }

    /// The id of `text`, assigning one if it is new.
    ///
    /// # Errors
    /// More than `u32::MAX` strings; a new string after [`Strings::seal`]
    /// (unsupported).
    pub fn intern(&self, text: &str) -> Result<u32, crate::Error> {
        if let Some(&id) = self
            .inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .ids
            .get(text)
        {
            return Ok(id);
        }
        let mut inner = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(&id) = inner.ids.get(text) {
            return Ok(id);
        }
        if self.sealed.load(AtomicOrdering::Acquire) {
            return Err(crate::Error::unsupported(format!(
                "the string or name {text:?}, which TLC first meets after start-up: its place in TLC's string order depends on evaluation order (--compat=tlc)"
            )));
        }
        let id = u32::try_from(inner.texts.len())
            .map_err(|_| crate::Error::unsupported("more than 2^32 distinct strings"))?;
        let text: Arc<str> = text.into();
        inner.payload_bytes += text.len();
        inner.longest_bytes = inner.longest_bytes.max(text.len());
        inner.texts.push(text.clone());
        inner.ids.insert(text, id);
        Ok(id)
    }

    /// The text of an id.
    #[must_use]
    pub fn text(&self, id: u32) -> Option<Arc<str>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .get(id as usize)
            .cloned()
    }

    /// The texts of the ids from `start` on, in id order, under one read
    /// lock: a worker's heap copies them once instead of locking the table
    /// on every comparison ([`crate::heap::Heap::cmp`]).
    #[must_use]
    pub fn texts_from(&self, start: usize) -> Vec<Arc<str>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .get(start..)
            .map(<[Arc<str>]>::to_vec)
            .unwrap_or_default()
    }

    /// Compares two ids in the table's order: by text, or by token.
    #[must_use]
    pub fn cmp(&self, a: u32, b: u32) -> std::cmp::Ordering {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        if self.order == StringOrder::Token {
            return a.cmp(&b);
        }
        let inner = self.inner.read().unwrap_or_else(PoisonError::into_inner);
        match (inner.texts.get(a as usize), inner.texts.get(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => a.cmp(&b),
        }
    }

    /// The dense rank of each id in the table's order, for the ids below
    /// the result's length: comparing two ranks compares the two strings
    /// as [`Strings::cmp`] does.
    ///
    /// Ranks are rebuilt here, not on every intern, so the ids interned
    /// since the last rebuild may lie past the result; the caller compares
    /// those by text. Interning shifts ranks but never reorders two ids
    /// that were ranked together, so an earlier result still orders every
    /// pair it covers.
    #[must_use]
    pub fn ranks(&self) -> Arc<[u32]> {
        {
            let inner = self.inner.read().unwrap_or_else(PoisonError::into_inner);
            if !rank_stale(inner.ranks.len(), inner.texts.len()) {
                return inner.ranks.clone();
            }
        }
        let mut inner = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        if rank_stale(inner.ranks.len(), inner.texts.len()) {
            // Ids are issued as `u32`, so their count fits unless the
            // table is full; a full table keeps its earlier ranks.
            if let Ok(len) = u32::try_from(inner.texts.len()) {
                let mut order: Vec<u32> = (0..len).collect();
                if self.order == StringOrder::Text {
                    let texts = &inner.texts;
                    order.sort_unstable_by(|&a, &b| texts[a as usize].cmp(&texts[b as usize]));
                }
                let mut ranks = vec![0; order.len()];
                for (rank, id) in (0u32..).zip(order) {
                    ranks[id as usize] = rank;
                }
                inner.ranks = ranks.into();
            }
        }
        inner.ranks.clone()
    }

    /// The UTF-8 length of the longest text in the table so far. The table
    /// is append-only, so every id it has issued names a text at most this
    /// long: it bounds the work of one scan over any one current string.
    #[must_use]
    pub fn longest_text_bytes(&self) -> usize {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .longest_bytes
    }

    /// The number of strings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Measures known dynamic storage without counting shared text twice.
    /// The index's opaque bucket allocation is reported as logical capacity,
    /// rather than inventing a physical byte count from that capacity.
    #[must_use]
    pub fn memory_usage(&self) -> StringsMemory {
        let inner = self.inner.read().unwrap_or_else(PoisonError::into_inner);
        StringsMemory {
            payload_bytes: inner.payload_bytes,
            text_slots_bytes: inner.texts.capacity() * std::mem::size_of::<Arc<str>>(),
            index_entries_bytes: inner.ids.len()
                * (std::mem::size_of::<Arc<str>>() + std::mem::size_of::<u32>()),
            index_capacity: inner.ids.capacity(),
            strings: inner.texts.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn memory_counts_unique_utf8_and_exposes_opaque_index_capacity() {
        let table = Strings::default();
        let _ = table.intern("alpha");
        let _ = table.intern("β");
        let _ = table.intern("");
        let before = table.memory_usage();
        assert_eq!(before.payload_bytes, 7);
        assert_eq!(before.strings, 3);
        assert!(before.index_capacity >= 3);
        assert!(before.text_slots_bytes >= 3 * std::mem::size_of::<Arc<str>>());
        let reader = table.text(0);
        let _ = table.intern("alpha");
        assert_eq!(table.memory_usage(), before, "shared strings count once");
        drop(reader);
        {
            let mut inner = table.inner.write().unwrap_or_else(PoisonError::into_inner);
            inner.texts.reserve(100);
            inner.ids.reserve(100);
        }
        let after = table.memory_usage();
        assert_eq!(after.payload_bytes, before.payload_bytes);
        assert_eq!(after.index_entries_bytes, before.index_entries_bytes);
        assert!(after.text_slots_bytes > before.text_slots_bytes);
        assert!(after.index_capacity > before.index_capacity);
        assert_eq!(
            after
                .known_retained_bytes()
                .and_then(|n| n.checked_sub(after.text_slots_bytes)),
            before
                .known_retained_bytes()
                .and_then(|n| n.checked_sub(before.text_slots_bytes)),
            "opaque hash bucket bytes are not invented from logical capacity"
        );
    }

    #[test]
    fn a_sealed_table_refuses_new_strings_only() {
        let t = Strings::new(StringOrder::Token);
        assert!(t.preload(["zeta", "alpha"]).is_ok());
        t.seal();
        assert_eq!(t.intern("alpha").ok(), Some(1));
        assert!(t.intern("beta").is_err());
    }

    #[test]
    fn longest_text_bytes_grows_with_new_texts_only() {
        let t = Strings::new(StringOrder::Text);
        assert_eq!(t.longest_text_bytes(), 0);
        assert!(t.intern("abc").is_ok());
        assert!(t.intern("\u{e9}").is_ok());
        assert_eq!(t.longest_text_bytes(), 3);
        assert!(t.intern("abcdef").is_ok());
        assert!(t.intern("abc").is_ok());
        assert_eq!(t.longest_text_bytes(), 6);
    }

    #[test]
    fn text_order_ignores_encounter_order() {
        let t = Strings::new(StringOrder::Text);
        let (b, a) = (t.intern("b").unwrap_or(0), t.intern("a").unwrap_or(0));
        assert_eq!(t.cmp(a, b), Ordering::Less);
    }

    /// Ranks order every pair they cover as [`Strings::cmp`] does, also
    /// after strings are interned between and around ranked ones, and an
    /// earlier rank copy stays valid for the pairs it covers.
    #[test]
    fn ranks_follow_the_table_order_as_it_grows() {
        for order in [StringOrder::Text, StringOrder::Token] {
            let t = Strings::new(order);
            let mut snapshots = Vec::new();
            for text in ["m", "c", "x", "", "cc", "b", "\u{e9}", "z", "a", "mm"] {
                assert!(t.intern(text).is_ok());
                snapshots.push(t.ranks());
            }
            let n = u32::try_from(t.len()).unwrap_or(0);
            for ranks in &snapshots {
                for a in 0..n {
                    for b in 0..n {
                        if let (Some(x), Some(y)) = (ranks.get(a as usize), ranks.get(b as usize)) {
                            assert_eq!(x.cmp(y), t.cmp(a, b), "{order:?} {a} {b}");
                        }
                    }
                }
            }
            assert_eq!(snapshots.last().map(|r| r.len()), Some(t.len()));
        }
    }

    /// Past [`RANK_ALL_UP_TO`] strings, a few new strings stay unranked
    /// until enough accumulate, and ranking then covers them all.
    #[test]
    fn large_tables_rank_new_strings_in_batches() {
        let t = Strings::new(StringOrder::Text);
        for n in (0..2048).rev() {
            assert!(t.intern(&format!("s{n:05}")).is_ok());
        }
        assert_eq!(t.ranks().len(), 2048);
        assert!(t.intern("a").is_ok());
        assert_eq!(t.ranks().len(), 2048, "one new string waits");
        for n in 0..256 {
            assert!(t.intern(&format!("t{n}")).is_ok());
        }
        let ranks = t.ranks();
        assert_eq!(ranks.len(), t.len());
        let a = t.lookup("a").unwrap_or(0);
        assert_eq!(ranks.get(a as usize), Some(&0));
    }

    #[test]
    fn token_order_follows_preloaded_encounter_order() {
        let t = Strings::new(StringOrder::Token);
        assert!(t.preload(["zeta", "alpha", "mid"]).is_ok());
        let ids: Vec<u32> = ["alpha", "zeta", "mid", "new"]
            .iter()
            .map(|s| t.intern(s).unwrap_or(u32::MAX))
            .collect();
        assert_eq!(t.cmp(ids[1], ids[0]), Ordering::Less, "zeta was first");
        assert_eq!(t.cmp(ids[0], ids[2]), Ordering::Less);
        assert_eq!(t.cmp(ids[2], ids[3]), Ordering::Less, "runtime names last");
        // Preloading again with the same prefix is fine; after other names
        // it is refused.
        assert!(t.preload(["zeta", "alpha"]).is_ok());
        let late = Strings::new(StringOrder::Token);
        let _ = late.intern("x");
        assert!(late.preload(["y"]).is_err());
    }
}
