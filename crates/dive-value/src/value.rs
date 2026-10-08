//! The value representation.
//!
//! [`Value`] is 16 bytes, immutable, `Send + Sync`, and cheap to clone: scalars are
//! inline and compound values are shared behind an [`Arc`]. Compound values carry a
//! precomputed (or lazily cached) hash, so hashing is O(1) and unequal values usually
//! differ at the first hash comparison. Dropping a value is iterative, so any nesting
//! depth is safe.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, OnceLock};

use num_bigint::BigInt;

use crate::atom::{Atom, mix64};
use crate::ctx::{Ctx, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::int::Int;

/// A TLA+ value.
///
/// Integers built through [`Value::int`] are canonical: [`Value::Int`] for every value
/// in `i64` range and [`Value::BigInt`] only outside it. Equality, ordering, hashing
/// and fingerprints are numeric even for a hand-built `BigInt` that fits in an `i64`.
/// Functions are canonical by domain (see [`Fcn`]). Sets may be enumerated or lazy
/// (see [`Set`]); equality and hashing look through the representation.
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

/// Per-kind constants for [`Value`]'s hash.
const K_BOOL: u64 = 0x1f83_d9ab_fb41_bd6b;
const K_INT: u64 = 0x5be0_cd19_137e_2179;
pub(crate) const K_STR: u64 = 0x6a09_e667_f3bc_c908;
const K_MV: u64 = 0xbb67_ae85_84ca_a73b;
pub(crate) const K_SET: u64 = 0x3c6e_f372_fe94_f82b;
const K_FCN: u64 = 0xa54f_f53a_5f1d_36f1;

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

    /// An integer value from an [`Int`], in canonical form.
    #[must_use]
    pub fn int(i: Int) -> Self {
        match i {
            Int::Small(v) => Self::Int(v),
            Int::Big(b) => match Int::from_big((*b).clone()) {
                Int::Small(v) => Self::Int(v),
                Int::Big(_) => Self::BigInt(b),
            },
        }
    }

    /// The integer, if this is one, in canonical form.
    #[must_use]
    pub fn as_int(&self) -> Option<Int> {
        match self {
            Self::Int(v) => Some(Int::Small(*v)),
            Self::BigInt(b) => Some(Int::from_big((**b).clone())),
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
    /// enumerated set it equals, for example). Never changes how TLC's operations
    /// behave: hashing an unnormalized set does not normalize it.
    #[must_use]
    pub fn hash64(&self) -> u64 {
        match self {
            Self::Bool(b) => mix64(K_BOOL ^ u64::from(*b)),
            Self::Int(v) => int_hash(*v),
            Self::BigInt(b) => match self.as_int().and_then(|i| i.to_i64()) {
                Some(v) => int_hash(v),
                None => b
                    .to_signed_bytes_le()
                    .iter()
                    .fold(K_INT, |h, d| mix2(h, u64::from(*d))),
            },
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

/// The TLC printing of the value. Formatting never changes how TLC's operations
/// behave: it does not normalize unnormalized sets.
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match crate::print::to_debug_string(self) {
            Some(s) => f.write_str(&s),
            None => write!(f, "<{}>", self.kind_name()),
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
/// definition; membership is decided without enumerating, as in TLC. Equality,
/// ordering, hashing and fingerprints stream lazy sets in normalized order rather
/// than materializing them. Infinite sets refuse enumeration with
/// [`ErrorKind::NotEnumerable`].
#[derive(Clone)]
pub struct Set(pub(crate) Arc<SetInner>);

pub(crate) struct SetInner {
    pub(crate) order: ValueOrder,
    pub(crate) hash: OnceLock<u64>,
    /// The normalized elements, once computed. Only successes are cached, so an error
    /// is always computed under the caller's context.
    pub(crate) elems: OnceLock<Arc<[Value]>>,
    /// For `Raw` sets: whether TLC would consider the set normalized (`isNorm`). Set
    /// only by the operations that normalize in TLC, never by hashing, `==` or
    /// formatting.
    pub(crate) tlc_norm: AtomicBool,
    /// The cached FP64 fingerprint of this set alone (see [`crate::fp64::FpCache`]).
    pub(crate) fp: crate::fp64::FpCache,
    pub(crate) repr: SetRepr,
}

/// The source of an evaluator-driven lazy set (`{x \in S : P}` or `{e : x \in S}`).
///
/// `dive-value` stores and prints such sets but cannot evaluate them: `dive-eval`
/// implements this trait.
pub trait DeferredSet: Send + Sync + fmt::Debug {
    /// Which construct the set is.
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
    /// A stable description of the definition (for example the bound set and the
    /// values the predicate closes over). Two unenumerable deferred sets of the same
    /// kind are equal, and ordered, by their keys; enumerable ones by their elements.
    /// The default, an empty key, makes all unenumerable sets of one kind equal.
    fn key(&self) -> Vec<Value> {
        Vec::new()
    }
}

/// See [`DeferredSet::kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeferredKind {
    /// `{x \in S : P}`.
    Filter,
    /// `{e : x \in S}`.
    Map,
    /// `S \cup T` of sets TLC keeps lazy (`SetCupValue`).
    Cup,
    /// `S \cap T` of sets TLC keeps lazy (`SetCapValue`).
    Cap,
    /// `S \ T` of sets TLC keeps lazy (`SetDiffValue`).
    Diff,
    /// `UNION S` that TLC keeps lazy (`UnionValue`).
    Union,
}

pub(crate) enum SetRepr {
    /// Normalized elements (the elements live in `SetInner::elems`).
    Enum,
    /// Elements as constructed, normalized on demand (TLC's unnormalized
    /// `SetEnumValue`). Until TLC would normalize it, membership scans the elements
    /// linearly with TLC's equality.
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
    /// An enumerated set as written, normalized on demand.
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
    pub(crate) fn new(order: ValueOrder, repr: SetRepr) -> Self {
        Self(Arc::new(SetInner {
            order,
            hash: OnceLock::new(),
            elems: OnceLock::new(),
            tlc_norm: AtomicBool::new(false),
            fp: crate::fp64::FpCache::new(),
            repr,
        }))
    }

    /// Builds a set from elements already sorted and deduplicated in `order`.
    pub(crate) fn from_sorted(order: ValueOrder, elems: Vec<Value>) -> Self {
        let set = Self::new(order, SetRepr::Enum);
        let elems: Arc<[Value]> = elems.into();
        let _ = set.0.hash.set(set_hash(
            elems.iter().map(Value::hash64),
            elems.len() as u128,
        ));
        let _ = set.0.elems.set(elems);
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

    /// The elements in normalized order, enumerating if the set is lazy: TLC's
    /// `elements()`/`toSetEnum()`. An unnormalized (`Raw`) set counts as normalized
    /// afterwards, as in TLC.
    ///
    /// # Errors
    /// [`ErrorKind::NotEnumerable`] for an infinite or symbolic set,
    /// [`ErrorKind::TooLarge`] when the set is too large, and comparison errors from
    /// normalization.
    pub fn elements(&self, ctx: Ctx) -> Result<Arc<[Value]>> {
        if !matches!(
            self.0.repr,
            SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_)
        ) {
            // TLC's `toSetEnum` sizes the set first: its size errors come first.
            crate::enumerate::size(self, ctx)?;
        }
        let e = self.cached_elements(ctx)?;
        self.mark_normalized();
        Ok(e)
    }

    /// The elements in normalized order, without changing TLC's view of the set or
    /// of any set nested in it (issue #32): the operations TLC would perform to
    /// normalize run with [`quiet::Quiet`] active, so they mark nothing.
    pub(crate) fn quiet_elements(&self, ctx: Ctx) -> Result<Arc<[Value]>> {
        if let Some(e) = self.0.elems.get() {
            return Ok(e.clone());
        }
        let _quiet = quiet::Quiet::enter();
        self.cached_elements(ctx)
    }

    /// Enumerates and normalizes the set, caching the result unless a mark was
    /// suppressed while computing it. A suppressed mark means TLC's normalization
    /// would have normalized a nested set; the cache must not skip that side effect
    /// when a TLC operation later asks for the elements, so the result is
    /// recomputed then.
    fn cached_elements(&self, ctx: Ctx) -> Result<Arc<[Value]>> {
        if let Some(e) = self.0.elems.get() {
            return Ok(e.clone());
        }
        let before = quiet::suppressed();
        let e: Arc<[Value]> = crate::enumerate::enumerate(self, ctx)?.into();
        if quiet::suppressed() == before {
            Ok(self.0.elems.get_or_init(|| e).clone())
        } else {
            Ok(e)
        }
    }

    /// Whether TLC would treat the set as normalized (always, except for a `Raw` set
    /// that no normalizing operation has touched).
    pub(crate) fn is_tlc_normalized(&self) -> bool {
        match self.0.repr {
            SetRepr::Raw(_) => self.0.tlc_norm.load(AtomicOrdering::Acquire),
            _ => true,
        }
    }

    /// Records that TLC would now treat the set as normalized.
    pub(crate) fn mark_normalized(&self) {
        if matches!(self.0.repr, SetRepr::Raw(_)) && !self.is_tlc_normalized() {
            if quiet::active() {
                quiet::note_suppressed();
            } else {
                self.0.tlc_norm.store(true, AtomicOrdering::Release);
            }
        }
    }

    /// The number of elements (TLC's `size()`, which normalizes).
    ///
    /// # Errors
    /// As [`Set::elements`], plus TLC's size limits in [`IntMode::Tlc`](crate::IntMode).
    pub fn size(&self, ctx: Ctx) -> Result<u64> {
        crate::enumerate::size(self, ctx)
    }

    /// The order-independent hash (see [`Value::hash64`]).
    #[must_use]
    pub fn hash64(&self) -> u64 {
        *self
            .0
            .hash
            .get_or_init(|| crate::order::compute_set_hash(self))
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

    /// The domain and range, if this is `[S -> T]`.
    #[must_use]
    pub fn as_fcn_set(&self) -> Option<(&Value, &Value)> {
        match &self.0.repr {
            SetRepr::FcnSet(d, r) => Some((d, r)),
            _ => None,
        }
    }

    /// The fields and their sets, if this is `[a : S, ...]`, sorted in the set's
    /// order (TLC's `SetOfRcdsValue.sortByNames` under [`ValueOrder::Tlc`]).
    #[must_use]
    pub fn as_rcd_set(&self) -> Option<&[(Atom, Value)]> {
        match &self.0.repr {
            SetRepr::RcdSet(f) => Some(f),
            _ => None,
        }
    }

    /// The factors, if this is `S1 \X ... \X Sn`.
    #[must_use]
    pub fn as_product(&self) -> Option<&[Value]> {
        match &self.0.repr {
            SetRepr::Product(f) => Some(f),
            _ => None,
        }
    }

    /// Whether two handles share the same allocation.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The evaluator-driven source, if this is a deferred set.
    #[must_use]
    pub fn as_deferred(&self) -> Option<&Arc<dyn DeferredSet>> {
        match &self.0.repr {
            SetRepr::Deferred(d) => Some(d),
            _ => None,
        }
    }

    /// The components of a lazy set: the base of `SUBSET S` or `Seq(S)`, the domain
    /// and range of `[S -> T]`, the factors of `S1 \X ... \X Sn`, or the field sets of
    /// `[a : S, ...]` (with their names); empty for other kinds.
    #[must_use]
    pub fn components(&self) -> Vec<(Option<Atom>, Value)> {
        match &self.0.repr {
            SetRepr::Subset(b) | SetRepr::Seq(b) => vec![(None, b.clone())],
            SetRepr::FcnSet(d, r) => vec![(None, d.clone()), (None, r.clone())],
            SetRepr::Product(fs) => fs.iter().map(|f| (None, f.clone())).collect(),
            SetRepr::RcdSet(fs) => fs
                .iter()
                .map(|(a, f)| (Some(a.clone()), f.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The elements of an enumerated set as constructed, without normalizing it (TLC's
    /// `SetEnumValue.elems` before `normalize()`), or `None` for other kinds or a set
    /// TLC would already treat as normalized.
    #[must_use]
    pub fn raw_elements(&self) -> Option<&[Value]> {
        match &self.0.repr {
            SetRepr::Raw(e) if !self.is_tlc_normalized() => Some(e),
            _ => None,
        }
    }
}

/// Quiet mode: while a [`quiet::Quiet`] guard is alive on this thread, operations
/// that TLC performs with side effects (normalizing an unnormalized set) compute the
/// same results without recording the side effect. `==`, [`Hash`] and `Debug` run
/// quietly (ADR 0012).
pub(crate) mod quiet {
    use std::cell::Cell;

    thread_local! {
        static DEPTH: Cell<u32> = const { Cell::new(0) };
        static SUPPRESSED: Cell<u64> = const { Cell::new(0) };
    }

    /// Keeps quiet mode active on this thread until dropped.
    pub(crate) struct Quiet(());

    impl Quiet {
        pub(crate) fn enter() -> Self {
            DEPTH.with(|d| d.set(d.get().saturating_add(1)));
            Self(())
        }
    }

    impl Drop for Quiet {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }

    pub(crate) fn active() -> bool {
        DEPTH.with(Cell::get) > 0
    }

    /// The number of marks suppressed on this thread so far.
    pub(crate) fn suppressed() -> u64 {
        SUPPRESSED.with(Cell::get)
    }

    pub(crate) fn note_suppressed() {
        SUPPRESSED.with(|s| s.set(s.get().wrapping_add(1)));
    }
}

/// Sets with more elements than this hash by cardinality alone, so that hashing a
/// huge lazy set (`0..2147483646`) is O(1). Equality still compares the elements.
pub(crate) const HASH_ELEMENT_LIMIT: u128 = 1 << 16;

/// The order-independent hash of a set with the given element hashes (consumed only
/// when `len` is at most [`HASH_ELEMENT_LIMIT`]).
pub(crate) fn set_hash(hashes: impl Iterator<Item = u64>, len: u128) -> u64 {
    let sum = if len <= HASH_ELEMENT_LIMIT {
        hashes.fold(0u64, |acc, h| acc.wrapping_add(mix64(h)))
    } else {
        0x9e37_79b9_7f4a_7c15
    };
    #[allow(clippy::cast_possible_truncation, reason = "hash mixing")]
    mix2(K_SET ^ (len as u64), sum)
}

// ---------------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------------

/// A finite function. Cloning is a reference-count increment.
///
/// The representation is canonical by domain, so equal functions have equal
/// representations:
/// - domain `1..n` (including the empty domain): a **tuple** (TLC `TupleValue`);
/// - a non-empty domain of strings: a **record**, fields sorted;
/// - any other domain: a **general** function (TLC `FcnRcdValue`), pairs sorted by
///   domain element.
///
/// A record also remembers whether it is a record *literal* (TLC `RecordValue`: built
/// by `[f |-> e]` or enumerated from `[f : S]`) or a function that happens to have a
/// string domain (TLC `FcnRcdValue`: built by `:>`, `@@`, `[x \in S |-> e]` or
/// enumerated from `[S -> T]`). TLC compares and prints the two differently; equality,
/// hashing and fingerprints ignore the distinction.
///
/// Likewise a tuple remembers whether it is a tuple *literal* (TLC `TupleValue`: built
/// by `<<...>>`, `Append`, `\o` and the other sequence operators, or enumerated from
/// `S1 \X S2`) or a function with domain `1..n` built any other way (TLC
/// `FcnRcdValue`: `[i \in 1..n |-> e]`, `:>`, `@@`, or an element of `[S -> T]`). TLC
/// reports function-application errors and value kinds differently for the two
/// ([`Fcn::is_tuple_literal`]); equality, hashing and fingerprints ignore it.
#[derive(Clone)]
pub struct Fcn(pub(crate) Arc<FcnInner>);

pub(crate) struct FcnInner {
    pub(crate) hash: u64,
    /// The order records' fields and general domains are sorted in.
    pub(crate) order: ValueOrder,
    /// For a record: TLC `RecordValue` (a record literal) rather than a string-domain
    /// `FcnRcdValue`. For a tuple: TLC `TupleValue` rather than a `FcnRcdValue` with
    /// domain `1..n`.
    pub(crate) literal: bool,
    /// The cached FP64 fingerprint of this function alone (see
    /// [`crate::fp64::FpCache`]).
    pub(crate) fp: crate::fp64::FpCache,
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
        Self::tuple_as(elems, true)
    }

    /// A function with domain `1..n` that TLC keeps as a `FcnRcdValue`.
    pub(crate) fn tuple_fcn(elems: Vec<Value>) -> Self {
        Self::tuple_as(elems, false)
    }

    fn tuple_as(elems: Vec<Value>, literal: bool) -> Self {
        let hash = fcn_hash(
            elems
                .iter()
                .enumerate()
                .map(|(i, v)| (int_hash(i64::try_from(i).unwrap_or(i64::MAX) + 1), v)),
            elems.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            order: ValueOrder::default(),
            literal,
            fp: crate::fp64::FpCache::new(),
            repr: FcnRepr::Tuple(elems.into_boxed_slice()),
        }))
    }

    pub(crate) fn record(fields: Vec<(Atom, Value)>, order: ValueOrder, literal: bool) -> Self {
        let hash = fcn_hash(
            fields.iter().map(|(a, v)| (mix2(K_STR, a.text_hash()), v)),
            fields.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            order,
            literal,
            fp: crate::fp64::FpCache::new(),
            repr: FcnRepr::Record(fields.into_boxed_slice()),
        }))
    }

    pub(crate) fn general(pairs: Vec<(Value, Value)>, order: ValueOrder) -> Self {
        let hash = fcn_hash(pairs.iter().map(|(d, v)| (d.hash64(), v)), pairs.len());
        Self(Arc::new(FcnInner {
            hash,
            order,
            literal: false,
            fp: crate::fp64::FpCache::new(),
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

    /// Whether this is a record literal (TLC `RecordValue`), as opposed to a function
    /// that has a string domain (see [`Fcn`]).
    #[must_use]
    pub fn is_record_literal(&self) -> bool {
        self.0.literal && matches!(self.0.repr, FcnRepr::Record(_))
    }

    /// Whether this is a tuple literal (TLC `TupleValue`), as opposed to a function
    /// with domain `1..n` that TLC keeps as a `FcnRcdValue` (see [`Fcn`]).
    #[must_use]
    pub fn is_tuple_literal(&self) -> bool {
        self.0.literal && matches!(self.0.repr, FcnRepr::Tuple(_))
    }

    /// The order the fields or domain are sorted in.
    #[must_use]
    pub fn order(&self) -> ValueOrder {
        self.0.order
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
            (FcnRepr::Tuple(e), Value::Int(i)) => i
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| e.get(i)),
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
// Iterative drop
// ---------------------------------------------------------------------------------

impl SetInner {
    fn take_children(&mut self, out: &mut Vec<Value>) {
        match std::mem::replace(&mut self.repr, SetRepr::Enum) {
            SetRepr::Raw(e) | SetRepr::Product(e) => out.extend(e.into_vec()),
            SetRepr::Subset(v) | SetRepr::Seq(v) => out.push(v),
            SetRepr::FcnSet(d, r) => {
                out.push(d);
                out.push(r);
            }
            SetRepr::RcdSet(fs) => out.extend(fs.into_vec().into_iter().map(|(_, v)| v)),
            other => self.repr = other,
        }
        if let Some(mut e) = self.elems.take()
            && let Some(slice) = Arc::get_mut(&mut e)
        {
            for v in slice.iter_mut() {
                out.push(std::mem::replace(v, Value::FALSE));
            }
        }
    }
}

impl FcnInner {
    fn take_children(&mut self, out: &mut Vec<Value>) {
        match std::mem::replace(&mut self.repr, FcnRepr::Tuple(Box::default())) {
            FcnRepr::Tuple(e) => out.extend(e.into_vec()),
            FcnRepr::Record(fs) => out.extend(fs.into_vec().into_iter().map(|(_, v)| v)),
            FcnRepr::General(ps) => {
                for (d, v) in ps.into_vec() {
                    out.push(d);
                    out.push(v);
                }
            }
        }
    }
}

/// Drops values without recursion: each uniquely owned compound value is emptied
/// into a work list before it is freed.
fn drain(mut stack: Vec<Value>) {
    while let Some(v) = stack.pop() {
        match v {
            Value::Set(mut s) => {
                if let Some(inner) = Arc::get_mut(&mut s.0) {
                    inner.take_children(&mut stack);
                }
            }
            Value::Fcn(mut f) => {
                if let Some(inner) = Arc::get_mut(&mut f.0) {
                    inner.take_children(&mut stack);
                }
            }
            _ => {}
        }
    }
}

impl Drop for SetInner {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        self.take_children(&mut stack);
        drain(stack);
    }
}

impl Drop for FcnInner {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        self.take_children(&mut stack);
        drain(stack);
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
    /// Whether an error occurs is decided exactly as TLC decides it, by TLC's
    /// insertion sort in TLC's name order, in both [`ValueOrder`]s (with names interned
    /// in TLC's order); the result is then sorted in `ctx.order`.
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
    /// until TLC would normalize it. Membership tests scan the elements with TLC's
    /// equality, so `1 \in {1, "a"}` is `TRUE` while `{1, "a"} = {}` is an error, as in
    /// TLC.
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

    /// The tuple (sequence) `<<e1, ..., en>>` (TLC `TupleValue`).
    #[must_use]
    pub fn tuple(elems: Vec<Self>) -> Self {
        Self::Fcn(Fcn::tuple(elems))
    }

    /// The function with domain `1..n` mapping `i` to `elems[i - 1]`, as TLC builds it
    /// from `[i \in 1..n |-> e]` (a `FcnRcdValue`, not a `TupleValue`; see [`Fcn`]).
    /// Equal to [`Value::tuple`] of the same elements.
    #[must_use]
    pub fn tuple_fcn(elems: Vec<Self>) -> Self {
        Self::Fcn(Fcn::tuple_fcn(elems))
    }

    /// The record literal `[f1 |-> v1, ..., fn |-> vn]` (TLC `RecordValue`). With no
    /// fields this is the empty function `<<>>`.
    ///
    /// # Errors
    /// [`ErrorKind::Duplicate`] when a field repeats (TLC: `Field name f occurs
    /// multiple times in record.`).
    pub fn record(fields: Vec<(Atom, Self)>, ctx: Ctx) -> Result<Self> {
        if fields.is_empty() {
            return Ok(Self::tuple(Vec::new()));
        }
        Ok(Self::Fcn(Fcn::record(
            sort_fields(fields, ctx.order)?,
            ctx.order,
            true,
        )))
    }

    /// The function with the given `(argument, value)` pairs (TLC `FcnRcdValue`, as
    /// built by `:>`, `@@` or `[x \in S |-> e]`), in canonical form (see [`Fcn`]). The
    /// domain is normalized with TLC's comparability rules.
    ///
    /// # Errors
    /// Comparison errors from normalizing the domain, and [`ErrorKind::Duplicate`]
    /// when an argument repeats (TLC: `The value\nd\noccurs multiple times in the
    /// function domain.`).
    pub fn fcn(pairs: Vec<(Self, Self)>, ctx: Ctx) -> Result<Self> {
        crate::order::make_fcn(pairs, ctx)
    }
}

pub(crate) fn sort_fields(
    mut fields: Vec<(Atom, Value)>,
    order: ValueOrder,
) -> Result<Vec<(Atom, Value)>> {
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
