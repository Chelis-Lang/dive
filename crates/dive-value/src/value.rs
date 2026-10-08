//! The value representation.
//!
//! [`Value`] is 16 bytes, immutable, `Send + Sync`, and cheap to clone: scalars are
//! inline and compound values are shared behind an [`Arc`]. Compound values carry a
//! precomputed (or lazily cached) hash, so hashing is O(1) and unequal values usually
//! differ at the first hash comparison.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use num_bigint::BigInt;

use crate::atom::{Atom, mix64};
use crate::ctx::{Ctx, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::int::Int;

/// A TLA+ value.
///
/// Integers are canonical: [`Value::Int`] holds every value in `i64` range and
/// [`Value::BigInt`] only values outside it, so structural equality is numeric
/// equality. Functions are canonical by domain (see [`Fcn`]), so two equal functions
/// always have the same representation. Sets may be enumerated or lazy (see [`Set`]);
/// equality and hashing look through the representation.
#[derive(Clone)]
pub enum Value {
    /// `TRUE` or `FALSE`.
    Bool(bool),
    /// An integer in `i64` range.
    Int(i64),
    /// An integer outside `i64` range (big-integer mode only).
    BigInt(Arc<BigInt>),
    /// A string.
    Str(Atom),
    /// A model value: untyped, or typed when named `x_name` (TLC's rule).
    Model(Atom),
    /// A set.
    Set(Set),
    /// A function, including tuples, sequences and records.
    Fcn(Fcn),
}

/// The empty-function hash seed and per-kind constants for [`Value`]'s hash.
const K_BOOL: u64 = 0x1f83_d9ab_fb41_bd6b;
const K_INT: u64 = 0x5be0_cd19_137e_2179;
const K_STR: u64 = 0x6a09_e667_f3bc_c908;
const K_MV: u64 = 0xbb67_ae85_84ca_a73b;
const K_SET: u64 = 0x3c6e_f372_fe94_f82b;
const K_FCN: u64 = 0xa54f_f53a_5f1d_36f1;
const K_INF: u64 = 0x510e_527f_ade6_82d1;

pub(crate) fn mix2(a: u64, b: u64) -> u64 {
    mix64(a ^ b.rotate_left(29).wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

/// Hash of the integer `v`, shared by inline and big integers and by tuple domains.
pub(crate) fn int_hash(v: i64) -> u64 {
    #[allow(clippy::cast_sign_loss, reason = "bit reinterpretation for hashing")]
    mix64((v as u64) ^ K_INT)
}

impl Value {
    /// `TRUE`.
    pub const TRUE: Self = Self::Bool(true);
    /// `FALSE`.
    pub const FALSE: Self = Self::Bool(false);

    /// An integer value from an [`Int`].
    #[must_use]
    pub fn int(i: Int) -> Self {
        match i {
            Int::Small(v) => Self::Int(v),
            Int::Big(b) => Self::BigInt(b),
        }
    }

    /// The integer, if this is one.
    #[must_use]
    pub fn as_int(&self) -> Option<Int> {
        match self {
            Self::Int(v) => Some(Int::Small(*v)),
            Self::BigInt(b) => Some(Int::Big(b.clone())),
            _ => None,
        }
    }

    /// The boolean, if this is one.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The set, if this is one.
    #[must_use]
    pub const fn as_set(&self) -> Option<&Set> {
        match self {
            Self::Set(s) => Some(s),
            _ => None,
        }
    }

    /// The function, if this is one.
    #[must_use]
    pub const fn as_fcn(&self) -> Option<&Fcn> {
        match self {
            Self::Fcn(f) => Some(f),
            _ => None,
        }
    }

    /// The hash used by [`Hash`]: order-independent for sets and functions, so equal
    /// values hash equally whatever their representation (an interval and the
    /// enumerated set it equals, for example).
    #[must_use]
    pub fn hash64(&self) -> u64 {
        match self {
            Self::Bool(b) => mix64(K_BOOL ^ u64::from(*b)),
            Self::Int(v) => int_hash(*v),
            Self::BigInt(b) => {
                let mut h = K_INT;
                for d in b.to_signed_bytes_le() {
                    h = mix2(h, u64::from(d));
                }
                h
            }
            Self::Str(a) => mix2(K_STR, a.text_hash()),
            Self::Model(a) => mix2(K_MV, a.text_hash()),
            Self::Set(s) => s.hash64(),
            Self::Fcn(f) => f.0.hash,
        }
    }

    /// A short name of the value's kind, for diagnostics.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "boolean",
            Self::Int(_) | Self::BigInt(_) => "integer",
            Self::Str(_) => "string",
            Self::Model(_) => "model value",
            Self::Set(_) => "set",
            Self::Fcn(_) => "function",
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<Int> for Value {
    fn from(v: Int) -> Self {
        Self::int(v)
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash64());
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        crate::order::semantic_eq(self, other)
    }
}

impl Eq for Value {}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match crate::print::to_tlc_string(self, Ctx::default()) {
            Ok(s) => f.write_str(&s),
            Err(_) => write!(f, "<{}>", self.kind_name()),
        }
    }
}

// ---------------------------------------------------------------------------------
// Sets
// ---------------------------------------------------------------------------------

/// A set, enumerated or lazy. Cloning is a reference-count increment.
///
/// Enumerated sets are kept normalized: sorted in the run's [`ValueOrder`] without
/// duplicates. Lazy sets (`a..b`, `SUBSET S`, `[S -> T]`, `S \X T`, `[a : S]`, `Nat`,
/// `Int`, `STRING`, `Seq(S)`, and evaluator-driven filters and maps) keep their
/// definition; membership is decided without enumerating, as in TLC, and enumeration
/// happens at most once, on demand, in normalized order. Infinite sets refuse
/// enumeration with [`ErrorKind::NotEnumerable`].
#[derive(Clone)]
pub struct Set(pub(crate) Arc<SetInner>);

pub(crate) struct SetInner {
    pub(crate) order: ValueOrder,
    pub(crate) hash: OnceLock<u64>,
    /// Enumeration (or normalization, for `Raw`) cache.
    pub(crate) elems: OnceLock<Result<Arc<[Value]>>>,
    pub(crate) repr: SetRepr,
}

/// The source of an evaluator-driven lazy set (`{x \in S : P}` or `{e : x \in S}`).
///
/// `dive-value` stores and prints such sets but cannot evaluate them: `dive-eval`
/// implements this trait.
pub trait DeferredSet: Send + Sync + fmt::Debug {
    /// Whether the set is a filter (`{x \in S : P}`) or a map (`{e : x \in S}`).
    fn kind(&self) -> DeferredKind;
    /// Computes the set's elements in any order; `dive-value` normalizes them.
    ///
    /// # Errors
    /// Any evaluation error.
    fn elements(&self, ctx: Ctx) -> Result<Vec<Value>>;
    /// Decides membership without enumerating, if the source can.
    ///
    /// # Errors
    /// Any evaluation error.
    fn member(&self, elem: &Value, ctx: Ctx) -> Option<Result<bool>>;
}

/// See [`DeferredSet::kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeferredKind {
    /// `{x \in S : P}`.
    Filter,
    /// `{e : x \in S}`.
    Map,
}

pub(crate) enum SetRepr {
    /// Normalized elements (the elements live in `SetInner::elems`).
    Enum,
    /// Elements as constructed, normalized on demand (TLC's unnormalized
    /// `SetEnumValue`). Membership scans them linearly with TLC's equality.
    Raw(Box<[Value]>),
    Interval(Int, Int),
    Subset(Value),
    FcnSet(Value, Value),
    /// Fields sorted in the set's order, without duplicates.
    RcdSet(Box<[(Atom, Value)]>),
    Product(Box<[Value]>),
    Nat,
    Int,
    Strings,
    Seq(Value),
    Deferred(Arc<dyn DeferredSet>),
}

/// The kind of a set, as a plain enum for matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SetKind {
    /// An enumerated, normalized set.
    Enum,
    /// An enumerated set not yet normalized.
    Raw,
    /// `a..b`.
    Interval,
    /// `SUBSET S`.
    Subset,
    /// `[S -> T]`.
    FcnSet,
    /// `[a : S, ...]`.
    RcdSet,
    /// `S1 \X ... \X Sn`.
    Product,
    /// `Nat`.
    Nat,
    /// `Int`.
    Int,
    /// `STRING`.
    Strings,
    /// `Seq(S)`.
    Seq,
    /// An evaluator-driven filter or map.
    Deferred,
}

impl Set {
    fn new(order: ValueOrder, repr: SetRepr) -> Self {
        Self(Arc::new(SetInner {
            order,
            hash: OnceLock::new(),
            elems: OnceLock::new(),
            repr,
        }))
    }

    /// Builds a set from elements already sorted and deduplicated in `order`.
    pub(crate) fn from_sorted(order: ValueOrder, elems: Vec<Value>) -> Self {
        let set = Self::new(order, SetRepr::Enum);
        let elems: Arc<[Value]> = elems.into();
        let _ = set.0.hash.set(set_hash(&elems));
        let _ = set.0.elems.set(Ok(elems));
        set
    }

    /// The set's kind.
    #[must_use]
    pub fn kind(&self) -> SetKind {
        match &self.0.repr {
            SetRepr::Enum => SetKind::Enum,
            SetRepr::Raw(_) => SetKind::Raw,
            SetRepr::Interval(..) => SetKind::Interval,
            SetRepr::Subset(_) => SetKind::Subset,
            SetRepr::FcnSet(..) => SetKind::FcnSet,
            SetRepr::RcdSet(_) => SetKind::RcdSet,
            SetRepr::Product(_) => SetKind::Product,
            SetRepr::Nat => SetKind::Nat,
            SetRepr::Int => SetKind::Int,
            SetRepr::Strings => SetKind::Strings,
            SetRepr::Seq(_) => SetKind::Seq,
            SetRepr::Deferred(_) => SetKind::Deferred,
        }
    }

    /// The order the set is sorted in.
    #[must_use]
    pub fn order(&self) -> ValueOrder {
        self.0.order
    }

    /// Whether the set is infinite (`Nat`, `Int`, `STRING`, `Seq(S)` for non-empty `S`,
    /// and lazy sets built from those).
    #[must_use]
    pub fn is_infinite(&self) -> bool {
        crate::enumerate::is_infinite(self)
    }

    /// The elements in normalized order, enumerating (once) if the set is lazy.
    ///
    /// # Errors
    /// [`ErrorKind::NotEnumerable`] for an infinite or symbolic set,
    /// [`ErrorKind::TooLarge`] when the set is too large, and comparison errors from
    /// normalization.
    pub fn elements(&self, ctx: Ctx) -> Result<Arc<[Value]>> {
        if let Some(r) = self.0.elems.get() {
            return r.clone();
        }
        let r = crate::enumerate::enumerate(self, ctx).map(Arc::from);

        self.0.elems.get_or_init(|| r).clone()
    }

    /// The number of elements.
    ///
    /// # Errors
    /// As [`Set::elements`], plus TLC's size limits in [`IntMode::Tlc`](crate::IntMode).
    pub fn size(&self, ctx: Ctx) -> Result<u64> {
        crate::enumerate::size(self, ctx)
    }

    /// The order-independent hash (see [`Value::hash64`]).
    #[must_use]
    pub fn hash64(&self) -> u64 {
        *self.0.hash.get_or_init(|| match &self.0.repr {
            SetRepr::Nat => mix2(K_INF, 1),
            SetRepr::Int => mix2(K_INF, 2),
            SetRepr::Strings => mix2(K_INF, 3),
            SetRepr::Seq(r) => mix2(K_INF ^ 4, r.hash64()),
            _ => match self.elements(Ctx {
                int_mode: crate::IntMode::Big,
                order: self.0.order,
            }) {
                Ok(e) => set_hash(&e),
                // A set that cannot be enumerated hashes by its definition; equality
                // then also falls back to the definition (`order::semantic_eq`).
                Err(_) => mix2(K_INF ^ 5, structural_hash(&self.0.repr)),
            },
        })
    }

    pub(crate) fn repr(&self) -> &SetRepr {
        &self.0.repr
    }

    /// The interval bounds, if this is `a..b`.
    #[must_use]
    pub fn as_interval(&self) -> Option<(&Int, &Int)> {
        match &self.0.repr {
            SetRepr::Interval(a, b) => Some((a, b)),
            _ => None,
        }
    }

    /// Whether two handles share the same allocation.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

fn structural_hash(repr: &SetRepr) -> u64 {
    match repr {
        SetRepr::Subset(b) => mix2(11, b.hash64()),
        SetRepr::FcnSet(d, r) => mix2(mix2(12, d.hash64()), r.hash64()),
        SetRepr::RcdSet(fs) => fs.iter().fold(13, |h, (a, v)| {
            h.wrapping_add(mix2(a.text_hash(), v.hash64()))
        }),
        SetRepr::Product(fs) => fs.iter().fold(14, |h, v| mix2(h, v.hash64())),
        SetRepr::Seq(r) => mix2(15, r.hash64()),
        SetRepr::Deferred(d) => mix2(16, d.kind() as u64),
        _ => 17,
    }
}

pub(crate) fn set_hash(elems: &[Value]) -> u64 {
    let sum = elems
        .iter()
        .fold(0u64, |acc, e| acc.wrapping_add(mix64(e.hash64())));
    mix2(K_SET ^ (elems.len() as u64), sum)
}

// ---------------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------------

/// A finite function. Cloning is a reference-count increment.
///
/// The representation is canonical by domain, so equal functions have equal
/// representations:
/// - domain `1..n` (including the empty domain): a **tuple** (TLC `TupleValue`);
/// - a non-empty domain of strings: a **record** (TLC `RecordValue`), fields sorted;
/// - any other domain: a **general** function (TLC `FcnRcdValue`), pairs sorted by
///   domain element.
///
/// TLC prints by domain too, so the canonical form prints exactly as TLC prints the
/// same function (`FcnRcdValue.toString`: `isRcd`, `isTuple`).
#[derive(Clone)]
pub struct Fcn(pub(crate) Arc<FcnInner>);

pub(crate) struct FcnInner {
    pub(crate) hash: u64,
    pub(crate) repr: FcnRepr,
}

pub(crate) enum FcnRepr {
    Tuple(Box<[Value]>),
    Record(Box<[(Atom, Value)]>),
    General(Box<[(Value, Value)]>),
}

/// The shape of a function's domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FcnKind {
    /// Domain `1..n`, including the empty function.
    Tuple,
    /// A non-empty domain of strings.
    Record,
    /// Any other domain.
    General,
}

fn fcn_hash<'a>(pairs: impl Iterator<Item = (u64, &'a Value)>, len: usize) -> u64 {
    let sum = pairs.fold(0u64, |acc, (dh, v)| acc.wrapping_add(mix2(dh, v.hash64())));
    mix2(K_FCN ^ (len as u64), sum)
}

impl Fcn {
    pub(crate) fn tuple(elems: Vec<Value>) -> Self {
        let hash = fcn_hash(
            elems
                .iter()
                .enumerate()
                .map(|(i, v)| (int_hash(i64::try_from(i).unwrap_or(i64::MAX) + 1), v)),
            elems.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            repr: FcnRepr::Tuple(elems.into_boxed_slice()),
        }))
    }

    pub(crate) fn record(fields: Vec<(Atom, Value)>) -> Self {
        let hash = fcn_hash(
            fields.iter().map(|(a, v)| (mix2(K_STR, a.text_hash()), v)),
            fields.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            repr: FcnRepr::Record(fields.into_boxed_slice()),
        }))
    }

    pub(crate) fn general(pairs: Vec<(Value, Value)>) -> Self {
        let hash = fcn_hash(pairs.iter().map(|(d, v)| (d.hash64(), v)), pairs.len());
        Self(Arc::new(FcnInner {
            hash,
            repr: FcnRepr::General(pairs.into_boxed_slice()),
        }))
    }

    /// The domain's shape.
    #[must_use]
    pub fn kind(&self) -> FcnKind {
        match &self.0.repr {
            FcnRepr::Tuple(_) => FcnKind::Tuple,
            FcnRepr::Record(_) => FcnKind::Record,
            FcnRepr::General(_) => FcnKind::General,
        }
    }

    /// The number of domain elements.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.0.repr {
            FcnRepr::Tuple(e) => e.len(),
            FcnRepr::Record(f) => f.len(),
            FcnRepr::General(p) => p.len(),
        }
    }

    /// Whether the domain is empty (the function `<<>>`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The elements, if the function is a tuple (domain `1..n`).
    #[must_use]
    pub fn as_tuple(&self) -> Option<&[Value]> {
        match &self.0.repr {
            FcnRepr::Tuple(e) => Some(e),
            _ => None,
        }
    }

    /// The fields in order, if the function is a record.
    #[must_use]
    pub fn as_record(&self) -> Option<&[(Atom, Value)]> {
        match &self.0.repr {
            FcnRepr::Record(f) => Some(f),
            _ => None,
        }
    }

    /// The `i`-th domain element and its value, in normalized domain order.
    #[must_use]
    pub fn entry(&self, i: usize) -> Option<(Value, &Value)> {
        match &self.0.repr {
            FcnRepr::Tuple(e) => e
                .get(i)
                .map(|v| (Value::Int(i64::try_from(i).unwrap_or(i64::MAX) + 1), v)),
            FcnRepr::Record(f) => f.get(i).map(|(a, v)| (Value::Str(a.clone()), v)),
            FcnRepr::General(p) => p.get(i).map(|(d, v)| (d.clone(), v)),
        }
    }

    /// Iterates over `(domain element, value)` in normalized domain order.
    pub fn iter(&self) -> impl Iterator<Item = (Value, &Value)> + '_ {
        (0..self.len()).filter_map(move |i| self.entry(i))
    }

    /// Applies the function to `arg`, returning `None` when `arg` is not in the domain.
    #[must_use]
    pub fn apply(&self, arg: &Value) -> Option<&Value> {
        match (&self.0.repr, arg) {
            (FcnRepr::Tuple(e), Value::Int(i)) => {
                usize::try_from(*i - 1).ok().and_then(|i| e.get(i))
            }
            (FcnRepr::Record(f), Value::Str(a)) => f.iter().find(|(n, _)| n == a).map(|(_, v)| v),
            (FcnRepr::General(p), _) => p.iter().find(|(d, _)| d == arg).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Whether two handles share the same allocation.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// ---------------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------------

impl Value {
    /// A string, interned in `names`.
    #[must_use]
    pub fn string(names: &crate::Interner, text: &str) -> Self {
        Self::Str(names.intern(text))
    }

    /// A model value, interned in `names`.
    #[must_use]
    pub fn model(names: &crate::Interner, name: &str) -> Self {
        Self::Model(names.intern(name))
    }

    /// A normalized set of `elems`.
    ///
    /// Normalization applies TLC's comparability rules: `{1, "a"}` is an error, with
    /// the message TLC gives (`Attempted to compare string "a" with non-string:\n1`).
    /// Under [`ValueOrder::Tlc`] the error is the one TLC reports, because the
    /// comparisons are made in TLC's order whenever an error is possible.
    ///
    /// When elements are equal but differently represented (`1..3` and `{1, 2, 3}`),
    /// the first one is kept, as in TLC.
    ///
    /// # Errors
    /// [`ErrorKind::Incomparable`] and friends, as in TLC.
    pub fn set(elems: Vec<Self>, ctx: Ctx) -> Result<Self> {
        crate::order::normalize_set(elems, ctx).map(|s| Self::Set(Set::from_sorted(ctx.order, s)))
    }

    /// A set exactly as TLC builds a set enumeration `{e1, ..., en}`: unnormalized
    /// until first needed. Membership tests scan the elements with TLC's equality, so
    /// `1 \in {1, "a"}` is `TRUE` while `{1, "a"} = {}` is an error, as in TLC.
    #[must_use]
    pub fn set_raw(elems: Vec<Self>, ctx: Ctx) -> Self {
        if elems.is_empty() {
            return Self::Set(Set::from_sorted(ctx.order, elems));
        }
        Self::Set(Set::new(ctx.order, SetRepr::Raw(elems.into_boxed_slice())))
    }

    /// The empty set.
    #[must_use]
    pub fn empty_set(ctx: Ctx) -> Self {
        Self::Set(Set::from_sorted(ctx.order, Vec::new()))
    }

    /// The interval `lo..hi` (empty when `hi < lo`).
    #[must_use]
    pub fn interval(lo: Int, hi: Int, ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Interval(lo, hi)))
    }

    /// `SUBSET base`.
    #[must_use]
    pub fn subset(base: Self, ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Subset(base)))
    }

    /// `[domain -> range]`.
    #[must_use]
    pub fn fcn_set(domain: Self, range: Self, ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::FcnSet(domain, range)))
    }

    /// `[f1 : S1, ..., fn : Sn]`.
    ///
    /// # Errors
    /// [`ErrorKind::Duplicate`] when a field repeats.
    pub fn record_set(fields: Vec<(Atom, Self)>, ctx: Ctx) -> Result<Self> {
        let fields = sort_fields(fields, ctx.order)?;
        Ok(Self::Set(Set::new(
            ctx.order,
            SetRepr::RcdSet(fields.into_boxed_slice()),
        )))
    }

    /// `S1 \X ... \X Sn` (`n >= 2` in TLA+ syntax; any `n` is accepted).
    #[must_use]
    pub fn product(factors: Vec<Self>, ctx: Ctx) -> Self {
        Self::Set(Set::new(
            ctx.order,
            SetRepr::Product(factors.into_boxed_slice()),
        ))
    }

    /// `Nat`.
    #[must_use]
    pub fn nat(ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Nat))
    }

    /// `Int`.
    #[must_use]
    pub fn int_set(ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Int))
    }

    /// `STRING`.
    #[must_use]
    pub fn string_set(ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Strings))
    }

    /// `Seq(range)`.
    #[must_use]
    pub fn seq_set(range: Self, ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Seq(range)))
    }

    /// `BOOLEAN`, which TLC represents as the enumerated set `{FALSE, TRUE}`.
    #[must_use]
    pub fn boolean_set(ctx: Ctx) -> Self {
        Self::Set(Set::from_sorted(ctx.order, vec![Self::FALSE, Self::TRUE]))
    }

    /// An evaluator-driven lazy set (`{x \in S : P}` or `{e : x \in S}`).
    #[must_use]
    pub fn deferred(source: Arc<dyn DeferredSet>, ctx: Ctx) -> Self {
        Self::Set(Set::new(ctx.order, SetRepr::Deferred(source)))
    }

    /// The tuple (sequence) `<<e1, ..., en>>`.
    #[must_use]
    pub fn tuple(elems: Vec<Self>) -> Self {
        Self::Fcn(Fcn::tuple(elems))
    }

    /// The record `[f1 |-> v1, ..., fn |-> vn]`. With no fields this is the empty
    /// function `<<>>`.
    ///
    /// # Errors
    /// [`ErrorKind::Duplicate`] when a field repeats (TLC: `Field name f occurs
    /// multiple times in record.`).
    pub fn record(fields: Vec<(Atom, Self)>, ctx: Ctx) -> Result<Self> {
        if fields.is_empty() {
            return Ok(Self::tuple(Vec::new()));
        }
        Ok(Self::Fcn(Fcn::record(sort_fields(fields, ctx.order)?)))
    }

    /// The function with the given `(argument, value)` pairs, in canonical form (see
    /// [`Fcn`]). The domain is normalized with TLC's comparability rules.
    ///
    /// # Errors
    /// Comparison errors from normalizing the domain, and [`ErrorKind::Duplicate`]
    /// when an argument repeats (TLC: `The value\nd\noccurs multiple times in the
    /// function domain.`).
    pub fn fcn(pairs: Vec<(Self, Self)>, ctx: Ctx) -> Result<Self> {
        crate::order::make_fcn(pairs, ctx)
    }
}

fn sort_fields(mut fields: Vec<(Atom, Value)>, order: ValueOrder) -> Result<Vec<(Atom, Value)>> {
    fields.sort_by(|a, b| a.0.cmp_with(&b.0, order));
    for w in fields.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(ValueError::new(
                ErrorKind::Duplicate,
                format!("Field name {} occurs multiple times in record.", w[1].0),
            ));
        }
    }
    Ok(fields)
}
