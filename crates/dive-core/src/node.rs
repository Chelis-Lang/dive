//! Compound node kinds and the uniform view over both regions
//! (`03-value-model.md` §4).
//!
//! Every compound is canonical when it is built:
//!
//! - a **set** is a sorted array of distinct element handles;
//! - a **function** with a contiguous integer domain `lo..lo+n-1` is an
//!   `IntFcn` holding only its values; sequences and tuples are the case
//!   `lo = 1`, and the empty function is `IntFcn` with `lo = 1`;
//! - any other function, records included, is a `Fcn`: its sorted keys, then
//!   its values.
//!
//! So equal values have equal node contents, and structural equality is a
//! comparison of arrays. The remaining kinds are symbolic sets, kept for
//! membership and streaming (S5); a finite symbolic set is materialized to a
//! `Set` before it is hashed, compared or stored in a state.
//!
//! M0 drops TLC's tuple/record literal flag (`dive-value`'s `literal` bit):
//! it is not observable in the spike models, and putting it in node identity
//! would break extensional equality. It returns as a lazy-view bit in M1.

use crate::handle::Handle;

/// The kind of a compound node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// A finite set: sorted distinct elements.
    Set,
    /// A function over `lo..lo+len-1`: its values.
    IntFcn,
    /// Any other function: `len` sorted keys, then `len` values.
    Fcn,
    /// `lo..hi`: `data = [hi]`.
    Interval,
    /// `SUBSET S`: `data = [S]`.
    Subset,
    /// `[D -> R]`: `data = [D, R]`.
    FcnSet,
    /// `[f1 : S1, ...]`: `n` sorted field names, then `n` sets.
    RecSet,
    /// `Seq(S)`: `data = [S]`.
    SeqSet,
    /// `Nat`.
    Nat,
    /// `Int`.
    Int,
    /// `S1 \X ... \X Sn`: the factors.
    Product,
}

impl Kind {
    /// Whether the kind is a set.
    #[must_use]
    pub const fn is_set(self) -> bool {
        !matches!(self, Self::IntFcn | Self::Fcn)
    }

    /// Whether the kind is a function.
    #[must_use]
    pub const fn is_fcn(self) -> bool {
        matches!(self, Self::IntFcn | Self::Fcn)
    }

    /// Whether the kind is a symbolic set.
    #[must_use]
    pub const fn is_symbolic(self) -> bool {
        !matches!(self, Self::Set | Self::IntFcn | Self::Fcn)
    }
}

/// A read-only view of a compound, from either region.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    /// The kind.
    pub kind: Kind,
    /// `IntFcn`: the domain's lower bound. `Interval`: its lower bound.
    pub lo: i64,
    /// The value hash.
    pub hash: u64,
    /// The raw additive sum the hash finalizes (sets and functions).
    pub sum: u64,
    /// The contents.
    pub data: &'a [Handle],
}

impl View<'_> {
    /// The number of elements (sets) or entries (functions).
    #[must_use]
    pub fn len(&self) -> usize {
        match self.kind {
            Kind::Fcn | Kind::RecSet => self.data.len() / 2,
            _ => self.data.len(),
        }
    }

    /// Whether the set or function is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values of a function.
    #[must_use]
    pub fn values(&self) -> &[Handle] {
        match self.kind {
            Kind::Fcn => &self.data[self.data.len() / 2..],
            _ => self.data,
        }
    }

    /// The keys of a `Fcn` (empty for other kinds).
    #[must_use]
    pub fn keys(&self) -> &[Handle] {
        match self.kind {
            Kind::Fcn => &self.data[..self.data.len() / 2],
            _ => &[],
        }
    }
}
