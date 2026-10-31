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
//! TLC's representation state that is observable under `--compat=tlc` is
//! kept out of identity as view bits ([`flags`]): a set enumeration that is
//! not yet normalized is a [`Kind::SetEnum`] node (flag [`flags::UNNORM`]),
//! a record literal is a `Fcn` with [`flags::RECORD`], and a tuple literal
//! an `IntFcn` with [`flags::TUPLE`] (`02-semantic-contract.md` C1, C2, C5).

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
    /// `A \cup B`, kept lazy: `data = [A, B]`.
    Union,
    /// `[x \in D |-> e]` before it is materialized: `lo` is the closure id
    /// (the evaluator's), `data = [D, captures...]`. Lazy.
    Lambda,
    /// `{x \in S : P}` before it is enumerated: `lo` is the closure id,
    /// `data = [S, captures...]`. Lazy.
    SetPred,
    /// An integer outside the inline 61-bit range: `lo` is the sign, `data`
    /// the magnitude's 32-bit digits (least significant first) as inline
    /// integers. Not a set (`int.rs`).
    BigInt,
    /// A set enumeration that TLC has not normalized yet
    /// (`SetEnumValue` with `isNorm` false): its elements in construction
    /// order, duplicates kept. Carries [`flags::UNNORM`]. A view, never an
    /// identity: it is normalized like any finite symbolic set
    /// (`03-value-model.md` §8).
    SetEnum,
}

/// [`View::flags`] bits. They describe a value's representation, never its
/// identity: hashing, equality, order and interning ignore them
/// (`03-value-model.md` §8, §11.5 rule 6).
pub mod flags {
    /// The value contains a [`super::Kind::Lambda`] or
    /// [`super::Kind::SetPred`] node, so only the evaluator can close it.
    pub const LAZY: u8 = 1;
    /// TLC's `isNorm` is false: the node is a [`super::Kind::SetEnum`] in
    /// construction order (`02-semantic-contract.md` C1).
    pub const UNNORM: u8 = 2;
    /// The function was built by a record literal (TLC's `RecordValue`),
    /// not by `:>`, `@@` or a constructor (`FcnRcdValue`)
    /// (`02-semantic-contract.md` C5). Set only on a `Fcn` node. Canonical
    /// identity interning drops it; its absence there is not provenance.
    pub const RECORD: u8 = 4;
    /// The function was built by a tuple literal (TLC's `TupleValue`), not
    /// as a function over `1..n` (`FcnRcdValue`) (`02-semantic-contract.md`
    /// C2). Set only on an `IntFcn` node with `lo = 1`. An identity handle
    /// without the bit is not proof of its original receiver class.
    pub const TUPLE: u8 = 8;
    /// A representation-preserving parent contains a child view. It belongs
    /// to the structural persistent class even when it is itself a general
    /// function. This is not a literal-class or normalization-history proof.
    pub const NESTED_VIEW: u8 = 16;
    /// The view bits: a node carrying one is a lazy view, so it is
    /// interned apart from identities (`03-value-model.md` §11.3).
    /// An actual owned construction snapshot; never inferred from set layout.
    /// Its origin and phase are retained apart from extensional identity (ADR0131).
    pub const HISTORY: u8 = 32;
    /// All observable representation bits.
    pub const VIEW: u8 = UNNORM | RECORD | TUPLE | NESTED_VIEW | HISTORY;
}

impl Kind {
    /// Whether the kind is a set.
    #[must_use]
    pub const fn is_set(self) -> bool {
        !matches!(self, Self::IntFcn | Self::Fcn | Self::Lambda | Self::BigInt)
    }

    /// Whether the kind is a function.
    #[must_use]
    pub const fn is_fcn(self) -> bool {
        matches!(self, Self::IntFcn | Self::Fcn)
    }

    /// Whether the kind is a symbolic set that the heap can enumerate on its
    /// own (not [`Kind::SetPred`], which needs the evaluator).
    #[must_use]
    pub const fn is_symbolic(self) -> bool {
        !matches!(
            self,
            Self::Set | Self::IntFcn | Self::Fcn | Self::Lambda | Self::SetPred | Self::BigInt
        )
    }

    /// Whether the kind is a closure the evaluator forces: a lazy function
    /// or a set predicate.
    #[must_use]
    pub const fn is_lazy(self) -> bool {
        matches!(self, Self::Lambda | Self::SetPred)
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
    /// Representation bits ([`flags`]); not part of identity.
    pub flags: u8,
    /// The contents.
    pub data: &'a [Handle],
}

impl View<'_> {
    /// Whether the value must be closed by the evaluator first
    /// ([`flags::LAZY`]).
    #[must_use]
    pub fn is_lazy(&self) -> bool {
        self.flags & flags::LAZY != 0
    }

    /// The representation's `isNorm` bit, not a certificate of construction
    /// history. Selected local literal views use their arena-owned phase;
    /// an alias can be normalized while retaining its original raw view.
    #[must_use]
    pub fn is_norm(&self) -> bool {
        self.flags & flags::UNNORM == 0
    }

    /// Whether a function was built by a record literal ([`flags::RECORD`]).
    #[must_use]
    pub fn is_record_literal(&self) -> bool {
        self.flags & flags::RECORD != 0
    }

    /// Whether a function was built by a tuple literal ([`flags::TUPLE`]).
    #[must_use]
    pub fn is_tuple_literal(&self) -> bool {
        self.flags & flags::TUPLE != 0
    }

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
