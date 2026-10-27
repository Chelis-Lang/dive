//! Handles: 64-bit tagged value words (`03-value-model.md` §2).
//!
//! The low three bits are a tag. Scalars are inline; compounds are an index
//! into the persistent store or into the owning worker's transient arena.

/// The tag of a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// A 61-bit signed integer.
    Int,
    /// A boolean.
    Bool,
    /// A string id into the string table.
    Str,
    /// A model value: its name's id in the string table. A model value and
    /// the string with the same text are different values.
    Model,
    /// A compound in the persistent store.
    Persistent,
    /// A compound in a worker's transient arena.
    Transient,
    /// The empty marker: an unassigned variable or an unforced thunk.
    Empty,
}

const TAG_BITS: u32 = 3;
const TAG_MASK: u64 = 0b111;
const T_INT: u64 = 0;
const T_BOOL: u64 = 1;
const T_STR: u64 = 2;
const T_MODEL: u64 = 3;
// Tag 6 is unused: big integers are compounds (`Kind::BigInt`).
const T_PERSISTENT: u64 = 4;
const T_TRANSIENT: u64 = 5;
const T_EMPTY: u64 = 7;

const SHARD_BITS: u32 = 8;
const CLASS_BITS: u32 = 2;
/// The number of store size classes (`store.rs`).
pub(crate) const CLASSES: usize = 4;
/// The store class of persistent lazy nodes (constants only).
pub(crate) const LAZY_CLASS: u8 = 3;
/// The bound on a node index within one shard and size class.
pub(crate) const MAX_CLASS_INDEX: u64 = (1 << 29) - 1;

/// The largest inline integer.
pub const INT_MAX: i64 = (1 << 60) - 1;
/// The smallest inline integer.
pub const INT_MIN: i64 = -(1 << 60);

/// A value word. Equal handles are equal values; for two persistent handles
/// the converse holds too, because the store is hash-consed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle(pub(crate) u64);

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.tag() {
            Tag::Int => write!(f, "Int({})", self.int_unchecked()),
            Tag::Bool => write!(f, "Bool({})", self.0 >> TAG_BITS != 0),
            Tag::Str => write!(f, "Str#{}", self.0 >> TAG_BITS),
            Tag::Model => write!(f, "Model#{}", self.0 >> TAG_BITS),
            Tag::Persistent => {
                let (s, c, i) = self.persistent_parts();
                write!(f, "P{s}:{c}:{i}")
            }
            Tag::Transient => {
                let (e, i) = self.transient_parts();
                write!(f, "T{e}:{i}")
            }
            Tag::Empty => write!(f, "Empty"),
        }
    }
}

impl Handle {
    /// The empty marker.
    pub const EMPTY: Self = Self(T_EMPTY);
    /// `TRUE`.
    pub const TRUE: Self = Self((1 << TAG_BITS) | T_BOOL);
    /// `FALSE`.
    pub const FALSE: Self = Self(T_BOOL);

    /// The raw bits, for hashing keys.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// The tag.
    #[must_use]
    pub const fn tag(self) -> Tag {
        match self.0 & TAG_MASK {
            T_INT => Tag::Int,
            T_BOOL => Tag::Bool,
            T_STR => Tag::Str,
            T_MODEL => Tag::Model,
            T_PERSISTENT => Tag::Persistent,
            T_TRANSIENT => Tag::Transient,
            _ => Tag::Empty,
        }
    }

    /// An inline integer, if it fits in 61 bits.
    #[must_use]
    pub const fn int(v: i64) -> Option<Self> {
        if v < INT_MIN || v > INT_MAX {
            None
        } else {
            #[allow(clippy::cast_sign_loss, reason = "two's complement packing")]
            Some(Self(((v << TAG_BITS) as u64) | T_INT))
        }
    }

    /// A boolean.
    #[must_use]
    pub const fn bool(b: bool) -> Self {
        if b { Self::TRUE } else { Self::FALSE }
    }

    /// A string by id.
    #[must_use]
    pub const fn string(id: u32) -> Self {
        Self(((id as u64) << TAG_BITS) | T_STR)
    }

    /// A model value by its name's string id.
    #[must_use]
    pub const fn model(id: u32) -> Self {
        Self(((id as u64) << TAG_BITS) | T_MODEL)
    }

    pub(crate) const fn persistent(shard: u8, class: u8, index: u64) -> Self {
        Self(
            (((index << (SHARD_BITS + CLASS_BITS))
                | ((class as u64) << SHARD_BITS)
                | shard as u64)
                << TAG_BITS)
                | T_PERSISTENT,
        )
    }

    pub(crate) const fn transient(epoch: u32, index: u32) -> Self {
        Self(((((epoch as u64) << 32) | index as u64) << TAG_BITS) | T_TRANSIENT)
    }

    /// The integer, if this is one.
    #[must_use]
    pub const fn as_int(self) -> Option<i64> {
        if self.0 & TAG_MASK == T_INT {
            Some(self.int_unchecked())
        } else {
            None
        }
    }

    #[allow(clippy::cast_possible_wrap, reason = "two's complement unpacking")]
    const fn int_unchecked(self) -> i64 {
        (self.0 as i64) >> TAG_BITS
    }

    /// The boolean, if this is one.
    #[must_use]
    pub const fn as_bool(self) -> Option<bool> {
        if self.0 & TAG_MASK == T_BOOL {
            Some(self.0 >> TAG_BITS != 0)
        } else {
            None
        }
    }

    /// The string id, if this is a string.
    #[must_use]
    pub const fn as_str_id(self) -> Option<u32> {
        if self.0 & TAG_MASK == T_STR {
            #[allow(clippy::cast_possible_truncation, reason = "ids are 32-bit")]
            Some((self.0 >> TAG_BITS) as u32)
        } else {
            None
        }
    }

    /// The name's string id, if this is a model value.
    #[must_use]
    pub const fn as_model_id(self) -> Option<u32> {
        if self.0 & TAG_MASK == T_MODEL {
            #[allow(clippy::cast_possible_truncation, reason = "ids are 32-bit")]
            Some((self.0 >> TAG_BITS) as u32)
        } else {
            None
        }
    }

    /// Whether this is a compound (persistent or transient).
    #[must_use]
    pub const fn is_compound(self) -> bool {
        matches!(self.0 & TAG_MASK, T_PERSISTENT | T_TRANSIENT)
    }

    /// Whether this is a transient compound.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        self.0 & TAG_MASK == T_TRANSIENT
    }

    /// Whether this is a persistent compound.
    #[must_use]
    pub const fn is_persistent(self) -> bool {
        self.0 & TAG_MASK == T_PERSISTENT
    }

    /// Whether this is a persistent lazy node (a lazy constant), which is
    /// not hash-consed by value: such handles must not be compared.
    #[must_use]
    /// Lazy values and view-flagged nodes ([`crate::node::flags::VIEW`])
    /// share this class: both are interned structurally, apart from
    /// identities.
    pub const fn is_lazy_persistent(self) -> bool {
        self.is_persistent() && self.persistent_parts().1 == LAZY_CLASS
    }

    /// Whether this is the empty marker.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 & TAG_MASK == T_EMPTY
    }

    /// The shard, size class and index of a persistent handle.
    #[allow(clippy::cast_possible_truncation, reason = "packed fields")]
    #[inline]
    pub(crate) const fn persistent_parts(self) -> (u8, u8, u64) {
        let p = self.0 >> TAG_BITS;
        (
            (p & 0xff) as u8,
            ((p >> SHARD_BITS) & ((1 << CLASS_BITS) - 1)) as u8,
            p >> (SHARD_BITS + CLASS_BITS),
        )
    }

    #[allow(clippy::cast_possible_truncation, reason = "packed fields")]
    pub(crate) const fn transient_parts(self) -> (u32, u32) {
        let p = self.0 >> TAG_BITS;
        ((p >> 32) as u32, p as u32)
    }
}

/// The 64-bit finalizer used for every hash (a `splitmix64`-style mixer).
#[must_use]
#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Combines a key hash and a value hash (an entry of a function or record).
#[must_use]
#[inline]
pub const fn mix2(a: u64, b: u64) -> u64 {
    mix64(a ^ b.rotate_left(29).wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

pub(crate) const K_BOOL: u64 = 0x1f83_d9ab_fb41_bd6b;
pub(crate) const K_INT: u64 = 0x5be0_cd19_137e_2179;
pub(crate) const K_STR: u64 = 0x6a09_e667_f3bc_c908;
pub(crate) const K_SET: u64 = 0x3c6e_f372_fe94_f82b;
pub(crate) const K_FCN: u64 = 0xa54f_f53a_5f1d_36f1;
pub(crate) const K_SYM: u64 = 0x510e_527f_ade6_82d1;
pub(crate) const K_MODEL: u64 = 0x1f83_d9ab_5be0_cd19;
/// The kind constant of a state hash (`03-value-model.md` §5).
pub const K_STATE: u64 = 0x9b05_688c_2b3e_6c1f;

/// `fin(K, len, Σ)`: the final mix of an additive hash.
#[must_use]
#[inline]
pub const fn fin(k: u64, len: u64, sum: u64) -> u64 {
    mix64(sum ^ mix64(k.wrapping_add(len.wrapping_mul(0x9e37_79b9_7f4a_7c15))))
}

/// The hash of a scalar handle (`None` for a compound).
#[must_use]
#[inline]
pub const fn scalar_hash(h: Handle) -> Option<u64> {
    match h.0 & TAG_MASK {
        T_INT => Some(int_hash(h.int_unchecked())),
        T_BOOL => Some(mix64(K_BOOL ^ (h.0 >> TAG_BITS))),
        T_STR => Some(mix64(K_STR ^ (h.0 >> TAG_BITS))),
        T_MODEL => Some(mix64(K_MODEL ^ (h.0 >> TAG_BITS))),
        _ => None,
    }
}

/// The hash of an integer, shared by integer keys of functions.
#[must_use]
#[inline]
pub const fn int_hash(v: i64) -> u64 {
    #[allow(clippy::cast_sign_loss, reason = "bit reinterpretation for hashing")]
    mix64((v as u64) ^ K_INT)
}

/// The contribution of variable `i` with value hash `h` to a state's sum.
#[must_use]
#[inline]
pub const fn state_term(i: usize, h: u64) -> u64 {
    mix2(mix64(K_STATE ^ i as u64), h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_round_trip_at_the_bounds() {
        for v in [0, 1, -1, INT_MAX, INT_MIN, 12345, -9876] {
            let h = Handle::int(v).unwrap_or(Handle::EMPTY);
            assert_eq!(h.as_int(), Some(v));
            assert_eq!(h.tag(), Tag::Int);
        }
        assert!(Handle::int(INT_MAX + 1).is_none());
        assert!(Handle::int(INT_MIN - 1).is_none());
    }

    #[test]
    fn compound_parts_round_trip() {
        let p = Handle::persistent(200, 2, (1 << 40) + 7);
        assert_eq!(p.persistent_parts(), (200, 2, (1 << 40) + 7));
        assert!(p.is_persistent());
        let t = Handle::transient(0x1fff_ffff, 0xffff_fff0);
        assert_eq!(t.transient_parts(), (0x1fff_ffff, 0xffff_fff0));
        assert!(t.is_transient());
        assert_eq!(Handle::TRUE.as_bool(), Some(true));
        assert_eq!(Handle::FALSE.as_bool(), Some(false));
        assert_eq!(Handle::string(9).as_str_id(), Some(9));
        assert!(Handle::EMPTY.is_empty());
    }
}
