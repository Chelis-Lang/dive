//! In a large fingerprint shard, the shard and the home slot already identify
//! part of each key. A bounded displacement restores the home slot exactly.

use std::collections::HashSet;

const SHARD_MASK: u64 = (1 << 56) - 1;
const MAX_DISPLACEMENT: usize = 14;

#[derive(Debug)]
struct Packed<const WIDTH: usize> {
    slots: Vec<[u8; WIDTH]>,
    home_bits: u32,
    overflow: HashSet<u64>,
}

enum Position<const WIDTH: usize> {
    Present,
    Empty(usize, [u8; WIDTH]),
    Overflow,
}

impl<const WIDTH: usize> Packed<WIDTH> {
    fn new(capacity: usize) -> Self {
        let home_bits = capacity.trailing_zeros();
        debug_assert!(
            capacity.is_power_of_two() && 60_usize.saturating_sub(home_bits as usize) <= 8 * WIDTH
        );
        Self {
            slots: vec![[0; WIDTH]; capacity],
            home_bits,
            overflow: HashSet::new(),
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "only the low home bits select a slot"
    )]
    fn position(&self, key: u64) -> Position<WIDTH> {
        let mask = self.slots.len() - 1;
        let home = key as usize & mask;
        let mut tag = Self::base_tag(key, self.home_bits);
        for delta in 0..=MAX_DISPLACEMENT {
            let index = (home + delta) & mask;
            // Every occupied tag has a nonzero low nibble.
            tag[0] += 1;
            let stored = self.slots[index];
            if stored[0] == 0 {
                return Position::Empty(index, tag);
            }
            if stored == tag {
                return Position::Present;
            }
        }
        Position::Overflow
    }

    fn base_tag(key: u64, bits: u32) -> [u8; WIDTH] {
        let suffix = ((key & SHARD_MASK) >> bits) << 4;
        let mut tag = [0; WIDTH];
        tag.copy_from_slice(&suffix.to_le_bytes()[..WIDTH]);
        tag
    }

    fn restore(prefix: u64, bits: u32, index: usize, tag: [u8; WIDTH]) -> u64 {
        let delta = usize::from((tag[0] & 15) - 1);
        let home = index.wrapping_sub(delta) as u64 & ((1_u64 << bits) - 1);
        let mut word = [0; 8];
        word[..WIDTH].copy_from_slice(&tag);
        prefix | ((u64::from_le_bytes(word) >> 4) << bits) | home
    }

    fn contains(&self, key: u64) -> bool {
        match self.position(key) {
            Position::Present => true,
            Position::Empty(..) => false,
            Position::Overflow => self.overflow.contains(&key),
        }
    }

    fn insert(&mut self, key: u64) -> bool {
        match self.position(key) {
            Position::Present => false,
            Position::Empty(index, tag) => {
                self.slots[index] = tag;
                true
            }
            Position::Overflow => self.overflow.insert(key),
        }
    }

    fn reinsert(self, prefix: u64, target: &mut Slots) {
        for (index, tag) in self.slots.into_iter().enumerate() {
            if tag[0] != 0 {
                target.insert(Self::restore(prefix, self.home_bits, index, tag));
            }
        }
        for key in self.overflow {
            target.insert(key);
        }
    }
}

#[derive(Debug)]
enum Slots {
    Words(Vec<u64>),
    Six(Packed<6>),
    Five(Packed<5>),
    Four(Packed<4>),
}

impl Slots {
    fn new(size: usize) -> Self {
        match size.trailing_zeros() {
            0..12 => Self::Words(vec![0; size]),
            12..20 => Self::Six(Packed::new(size)),
            20..28 => Self::Five(Packed::new(size)),
            _ => Self::Four(Packed::new(size)),
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "only low slot bits are used"
    )]
    fn word_position(words: &[u64], key: u64) -> usize {
        let mask = words.len() - 1;
        let mut index = key as usize & mask;
        while words[index] != 0 && words[index] != key {
            index = (index + 1) & mask;
        }
        index
    }

    fn contains(&self, key: u64) -> bool {
        match self {
            Self::Words(words) => words[Self::word_position(words, key)] == key,
            Self::Six(p) => p.contains(key),
            Self::Five(p) => p.contains(key),
            Self::Four(p) => p.contains(key),
        }
    }

    fn insert(&mut self, key: u64) -> bool {
        match self {
            Self::Words(words) => {
                let i = Self::word_position(words, key);
                let new = words[i] == 0;
                words[i] = key;
                new
            }
            Self::Six(p) => p.insert(key),
            Self::Five(p) => p.insert(key),
            Self::Four(p) => p.insert(key),
        }
    }

    fn reinsert(self, prefix: u64, target: &mut Self) {
        match self {
            Self::Words(words) => {
                for key in words.into_iter().filter(|&k| k != 0) {
                    target.insert(key);
                }
            }
            Self::Six(p) => p.reinsert(prefix, target),
            Self::Five(p) => p.reinsert(prefix, target),
            Self::Four(p) => p.reinsert(prefix, target),
        }
    }
}

/// Resizing one shard at a time bounds the overlap between old and new slots.
#[derive(Debug)]
pub(super) struct FingerprintShard {
    slots: Slots,
    capacity: usize,
    used: usize,
}

impl FingerprintShard {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            slots: Slots::new(capacity),
            capacity,
            used: 0,
        }
    }

    pub(super) fn contains(&self, key: u64) -> bool {
        self.slots.contains(key)
    }

    pub(super) fn insert(&mut self, key: u64) -> bool {
        if self.used == self.capacity - self.capacity / 4 {
            if self.contains(key) {
                return false;
            }
            let next = self.capacity * 2;
            let old = std::mem::replace(&mut self.slots, Slots::new(next));
            old.reinsert(key & !SHARD_MASK, &mut self.slots);
            self.capacity = next;
        }
        let fresh = self.slots.insert(key);
        self.used += usize::from(fresh);
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_restores_the_full_key_through_wraparound() {
        fn check<const W: usize>(bits: u32) {
            let size = 1usize << bits;
            for prefix in [0, 19 << 56, 255 << 56] {
                for payload in [0, SHARD_MASK, 0x00de_adbe_ef01_2345] {
                    for home in [0, 7, size - 2, size - 1] {
                        let key = prefix | (payload & !((size - 1) as u64)) | home as u64;
                        for delta in 0..=MAX_DISPLACEMENT {
                            let mut tag = Packed::<W>::base_tag(key, bits);
                            tag[0] += u8::try_from(delta + 1).unwrap_or_default();
                            assert_eq!(
                                Packed::<W>::restore(
                                    prefix,
                                    bits,
                                    (home + delta) & (size - 1),
                                    tag
                                ),
                                key
                            );
                        }
                    }
                }
            }
        }
        check::<6>(12);
        check::<6>(19);
        check::<5>(20);
        check::<5>(27);
        check::<4>(28);
    }

    #[test]
    fn overflow_and_growth_preserve_dense_collisions_and_duplicate_queries() {
        let mut shard = FingerprintShard::new(256);
        let prefix = 0xfa00_0000_0000_0000;
        let keys: Vec<_> = (1..12_000_u64).map(|n| prefix | (n << 24) | 4095).collect();
        for &key in &keys {
            assert!(!shard.contains(key));
            assert!(shard.insert(key));
            assert!(!shard.insert(key));
            assert!(shard.contains(key));
        }
        for key in keys {
            assert!(shard.contains(key));
            assert!(!shard.contains(key ^ (1 << 40)));
        }
        assert!(matches!(shard.slots, Slots::Six(ref p) if !p.overflow.is_empty()));
    }
}
