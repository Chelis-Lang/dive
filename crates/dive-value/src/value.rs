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
use crate::function_domain::{DomainInner, GeneralFcn};
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
pub trait DeferredSet: Send + Sync + fmt::Debug + std::any::Any {
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
    /// The source as [`std::any::Any`], so that its owner can recover its
    /// own type (the evaluator reproduces TLC's unnormalized enumeration of
    /// its lazy sets). The default, `None`, hides it.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
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

    /// Stored explicit elements, without normalization, enumeration or callbacks.
    #[must_use]
    pub fn stored_elements(&self) -> Option<&[Value]> {
        match &self.0.repr {
            SetRepr::Enum => self.0.elems.get().map(AsRef::as_ref),
            SetRepr::Raw(e) => Some(e),
            _ => None,
        }
    }

    /// Whether normalized element storage has already been cached.
    /// This reads storage only; it invokes no provider and grants no authority
    /// to reproduce a deferred provider's conversion or callback history.
    #[must_use]
    pub fn has_cached_elements(&self) -> bool {
        self.0.elems.get().is_some()
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

    /// Streams a concrete set's raw TLC enumeration over concrete components,
    /// without sizing or materializing the complete compound result.
    ///
    /// Returns `None`, without normalization or callbacks, unless this is a
    /// finite explicit collection, interval, powerset, product, function set
    /// or record set whose complete component storage is concrete and in the
    /// same value order. Deferred or symbolic storage keeps its old route. All capability
    /// checks precede effects; this selects output storage, never admission.
    ///
    /// Constructor emptiness checks precede component normalization, in TLC's
    /// order. Nonempty components are materialized before the first output, so
    /// this does not make a huge individual component incremental. Output order
    /// follows this set's construction order; callers must establish that it is
    /// their required exploration order. The full result is not cached.
    ///
    /// ```
    /// use dive_value::{Ctx, Value};
    /// let ctx = Ctx::dive();
    /// let base = Value::interval(1.into(), 31.into(), ctx);
    /// let Value::Set(powerset) = Value::subset(base, ctx) else {
    ///     return Err("SUBSET must produce a set".into());
    /// };
    /// let mut elements = powerset.concrete_element_stream(ctx)?
    ///     .ok_or("components need another enumeration route")?;
    /// assert_eq!(elements.next(), Some(Value::set(vec![], ctx)?));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Required component size/normalization errors. A failed capability check
    /// returns `None` rather than evaluating or refusing an unsupported provider.
    pub fn concrete_element_stream(&self, ctx: Ctx) -> Result<Option<crate::SetElementStream>> {
        crate::stream::concrete_raw(self, ctx)
    }

    /// Prepare a concrete powerset with typed, transactional integer positions.
    /// Non-powersets or unknown component storage decline before consumption.
    /// The required component size/normalization precedes its first empty value.
    ///
    /// # Errors
    /// The same required component errors as [`Self::concrete_element_stream`].
    pub fn concrete_subset_stream(&self, ctx: Ctx) -> Result<Option<crate::ConcreteSubsetStream>> {
        crate::stream::concrete_subset(self, ctx)
    }

    /// The elements in normalized order, without changing TLC's view of the set
    /// or of any set nested in it: [`Set::elements`] without its side effects.
    ///
    /// # Errors
    /// As [`Set::elements`].
    pub fn peek_elements(&self, ctx: Ctx) -> Result<Arc<[Value]>> {
        self.quiet_elements(ctx)
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
    /// that no normalizing operation has touched). This reads representation only;
    /// it neither enumerates nor changes the set. It is not a validation certificate.
    #[must_use]
    pub fn is_tlc_normalized(&self) -> bool {
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

    /// The address of the shared allocation: while both handles are alive,
    /// two handles have the same address exactly when they are
    /// [`ptr_eq`](Self::ptr_eq).
    #[must_use]
    pub fn addr(&self) -> usize {
        Arc::as_ptr(&self.0).addr()
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

    /// Counts the structural components returned by [`Self::components`],
    /// without cloning values, enumerating the set or invoking deferred callbacks.
    /// Bounded callers can check this before allocating a component work list.
    #[must_use]
    pub fn component_count(&self) -> usize {
        match &self.0.repr {
            SetRepr::Subset(_) | SetRepr::Seq(_) => 1,
            SetRepr::FcnSet(..) => 2,
            SetRepr::Product(fs) => fs.len(),
            SetRepr::RcdSet(fs) => fs.len(),
            _ => 0,
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

/// Whether this thread is normalizing a value without observable side effects.
///
/// Infallible equality, ordering and hashing can force deferred providers in
/// this mode. A cooperative evaluator must defer host interruption until this
/// unit returns: a transient interruption cannot stand in for a value or become
/// a cached structural hash. Quiet mode records normalization observability,
/// rather than whether an operation is infallible. Ordinary error propagation
/// is unchanged; cooperative interruption is deferred for the entire quiet
/// unit, including fallible peek work nested inside it.
#[must_use]
pub fn quiet_operation_active() -> bool {
    quiet::active()
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
    /// Order, literal receiver class and sealed constructor/source-hook facts.
    /// Replaces the old two bytes plus padding without growing the header.
    pub(crate) metadata: crate::receiver::FunctionMetadata,
    pub(crate) fp: crate::fp64::FpCache,
    pub(crate) repr: FcnRepr,
}

pub(crate) enum FcnRepr {
    Tuple(Box<[Value]>),
    Record(Box<[(Atom, Value)]>),
    General(GeneralFcn),
}

/// A conservative exact-domain lookup, without evaluating unknown keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainLookup {
    /// The original checked lookup is required.
    Unknown,
    /// Every possible comparison is error-free and the key is absent.
    Missing,
    /// The key is at this index in the function's stored range order.
    Found(usize),
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
        Self::tuple_metadata(
            elems,
            crate::receiver::FunctionMetadata::new(ValueOrder::default(), literal),
        )
    }

    pub(crate) fn tuple_metadata(
        elems: Vec<Value>,
        metadata: crate::receiver::FunctionMetadata,
    ) -> Self {
        let hash = fcn_hash(
            elems
                .iter()
                .enumerate()
                .map(|(i, v)| (int_hash(i64::try_from(i).unwrap_or(i64::MAX) + 1), v)),
            elems.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            metadata,
            fp: crate::fp64::FpCache::new(),
            repr: FcnRepr::Tuple(elems.into_boxed_slice()),
        }))
    }

    pub(crate) fn record(fields: Vec<(Atom, Value)>, order: ValueOrder, literal: bool) -> Self {
        Self::record_metadata(
            fields,
            crate::receiver::FunctionMetadata::new(order, literal),
        )
    }

    pub(crate) fn record_metadata(
        fields: Vec<(Atom, Value)>,
        metadata: crate::receiver::FunctionMetadata,
    ) -> Self {
        let hash = fcn_hash(
            fields.iter().map(|(a, v)| (mix2(K_STR, a.text_hash()), v)),
            fields.len(),
        );
        Self(Arc::new(FcnInner {
            hash,
            metadata,
            fp: crate::fp64::FpCache::new(),
            repr: FcnRepr::Record(fields.into_boxed_slice()),
        }))
    }

    pub(crate) fn general(pairs: Vec<(Value, Value)>, order: ValueOrder) -> Self {
        let hash = fcn_hash(pairs.iter().map(|(d, v)| (d.hash64(), v)), pairs.len());
        Self(Arc::new(FcnInner {
            hash,
            metadata: crate::receiver::FunctionMetadata::new(order, false),
            fp: crate::fp64::FpCache::new(),
            repr: FcnRepr::General(GeneralFcn::new(pairs, order)),
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
        self.0.metadata.literal() && matches!(self.0.repr, FcnRepr::Record(_))
    }

    /// Whether this is a tuple literal (TLC `TupleValue`), as opposed to a function
    /// with domain `1..n` that TLC keeps as a `FcnRcdValue` (see [`Fcn`]).
    #[must_use]
    pub fn is_tuple_literal(&self) -> bool {
        self.0.metadata.literal() && matches!(self.0.repr, FcnRepr::Tuple(_))
    }

    /// The order the fields or domain are sorted in.
    #[must_use]
    pub fn order(&self) -> ValueOrder {
        self.0.metadata.order()
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

    /// The `i`-th entry, borrowing stored keys where possible.
    #[must_use]
    pub fn entry_cow(&self, i: usize) -> Option<(std::borrow::Cow<'_, Value>, &Value)> {
        match &self.0.repr {
            FcnRepr::General(p) => p.get(i).map(|(d, v)| (std::borrow::Cow::Borrowed(d), v)),
            _ => self.entry(i).map(|(d, v)| (std::borrow::Cow::Owned(d), v)),
        }
    }

    /// An exact index or miss for eager integer/Cartesian domains. `Unknown` means
    /// the argument requires the caller's original checked comparison path.
    ///
    /// ```
    /// use dive_value::{Ctx, DomainLookup, Value};
    /// let Value::Fcn(f) = Value::fcn(
    ///     vec![(Value::Int(0), Value::TRUE), (Value::Int(1), Value::FALSE)],
    ///     Ctx::dive(),
    /// )? else { unreachable!() };
    /// assert_eq!(f.checked_domain_index(&Value::Int(1)), DomainLookup::Found(1));
    /// assert_eq!(f.checked_domain_index(&Value::Int(4)), DomainLookup::Missing);
    /// assert_eq!(f.checked_domain_index(&Value::TRUE), DomainLookup::Unknown);
    /// # Ok::<(), dive_value::ValueError>(())
    /// ```
    #[must_use]
    pub fn checked_domain_index(&self, argument: &Value) -> DomainLookup {
        match &self.0.repr {
            FcnRepr::General(p) => p.domain.checked_index(argument),
            _ => DomainLookup::Unknown,
        }
    }

    /// Cached TLC key positions for a proven provider-free general domain.
    /// This implementation view does not mark the function TLC-normalized.
    #[must_use]
    pub fn tlc_domain_indices(&self) -> Option<&[usize]> {
        match &self.0.repr {
            FcnRepr::General(p) => p.domain.tlc_order(),
            _ => None,
        }
    }

    /// Iterates over `(domain element, value)` in normalized domain order.
    pub fn iter(&self) -> impl Iterator<Item = (Value, &Value)> + '_ {
        (0..self.len()).filter_map(move |i| self.entry(i))
    }

    /// Iterates over the values in normalized domain order, without building
    /// the domain elements.
    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        let (tuple, record, general) = match &self.0.repr {
            FcnRepr::Tuple(e) => (Some(e.iter()), None, None),
            FcnRepr::Record(f) => (None, Some(f.iter().map(|(_, v)| v)), None),
            FcnRepr::General(p) => (None, None, Some(p.iter().map(|(_, v)| v))),
        };
        tuple
            .into_iter()
            .flatten()
            .chain(record.into_iter().flatten())
            .chain(general.into_iter().flatten())
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
            (FcnRepr::General(p), _) => match p.domain.checked_index(arg) {
                DomainLookup::Found(index) => p.values.get(index),
                DomainLookup::Missing => None,
                DomainLookup::Unknown => p.iter().find(|(d, _)| *d == arg).map(|(_, v)| v),
            },
            _ => None,
        }
    }

    /// Replaces one range value while preserving an already canonical domain.
    /// Returns `new` unchanged when rebuilding in `ctx` might normalize keys or
    /// change the function's representation; callers then use their original path.
    ///
    /// # Errors
    /// Returns the proposed value for an invalid index or an unproved domain.
    pub fn with_value_at(
        &self,
        i: usize,
        new: Value,
        ctx: Ctx,
    ) -> std::result::Result<Self, Value> {
        if i >= self.len() {
            return Err(new);
        }
        match &self.0.repr {
            FcnRepr::Tuple(values) => {
                let mut values = values.to_vec();
                values[i] = new;
                Ok(Self::tuple_as(values, self.0.metadata.literal()))
            }
            FcnRepr::Record(fields) => {
                if self.order() != ctx.order {
                    return Err(new);
                }
                let mut fields = fields.to_vec();
                fields[i].1 = new;
                Ok(Self::record(
                    fields,
                    self.order(),
                    self.0.metadata.literal(),
                ))
            }
            FcnRepr::General(general) => {
                if self.order() != ctx.order || !general.domain.canonical() {
                    return Err(new);
                }
                let mut values = general.values.to_vec();
                values[i] = new;
                let hash = fcn_hash(
                    general
                        .domain
                        .keys
                        .iter()
                        .zip(&values)
                        .map(|(d, v)| (d.hash64(), v)),
                    values.len(),
                );
                Ok(Self(Arc::new(FcnInner {
                    hash,
                    metadata: crate::receiver::FunctionMetadata::new(
                        self.order(),
                        self.0.metadata.literal(),
                    ),
                    fp: crate::fp64::FpCache::new(),
                    repr: FcnRepr::General(GeneralFcn {
                        domain: Arc::clone(&general.domain),
                        values: values.into_boxed_slice(),
                    }),
                })))
            }
        }
    }

    pub(crate) fn exact_domain_equals(&self, set: &Set) -> Option<bool> {
        match &self.0.repr {
            FcnRepr::General(general) => general.domain.matches_set(set),
            _ => None,
        }
    }

    /// Whether two handles share the same allocation.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The address of the shared allocation: while both handles are alive,
    /// two handles have the same address exactly when they are
    /// [`ptr_eq`](Self::ptr_eq).
    #[must_use]
    pub fn addr(&self) -> usize {
        Arc::as_ptr(&self.0).addr()
    }
}

// ---------------------------------------------------------------------------------
// Iterative drop
// ---------------------------------------------------------------------------------

/// Queues a child that would be freed with its parent; drops any other child
/// (a scalar, or a compound value shared elsewhere) in place, in O(1).
fn queue(out: &mut Vec<Value>, v: Value) {
    if frees_compound(&v) {
        out.push(v);
    }
}

impl SetInner {
    fn take_children(&mut self, out: &mut Vec<Value>) {
        match std::mem::replace(&mut self.repr, SetRepr::Enum) {
            SetRepr::Raw(e) | SetRepr::Product(e) => {
                for v in e {
                    queue(out, v);
                }
            }
            SetRepr::Subset(v) | SetRepr::Seq(v) => queue(out, v),
            SetRepr::FcnSet(d, r) => {
                queue(out, d);
                queue(out, r);
            }
            SetRepr::RcdSet(fs) => {
                for (_, v) in fs {
                    queue(out, v);
                }
            }
            other => self.repr = other,
        }
        if let Some(mut e) = self.elems.take()
            && let Some(slice) = Arc::get_mut(&mut e)
        {
            for v in slice.iter_mut() {
                queue(out, std::mem::replace(v, Value::FALSE));
            }
        }
    }
}

impl FcnInner {
    fn take_children(&mut self, out: &mut Vec<Value>) {
        match std::mem::replace(&mut self.repr, FcnRepr::Tuple(Box::default())) {
            FcnRepr::Tuple(e) => {
                for v in e {
                    queue(out, v);
                }
            }
            FcnRepr::Record(fs) => {
                for (_, v) in fs {
                    queue(out, v);
                }
            }
            FcnRepr::General(mut ps) => {
                for v in std::mem::take(&mut ps.values) {
                    queue(out, v);
                }
                if let Some(domain) = Arc::get_mut(&mut ps.domain) {
                    domain.take_children(out);
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

/// Whether dropping this handle would free a compound value, which the iterative
/// drop must then take apart. Scalars and shared compound values drop in O(1)
/// without recursion.
fn frees_compound(v: &Value) -> bool {
    match v {
        Value::Set(s) => Arc::strong_count(&s.0) == 1,
        Value::Fcn(f) => Arc::strong_count(&f.0) == 1,
        _ => false,
    }
}

impl SetInner {
    /// Whether any child this set owns would be freed with it (see
    /// [`frees_compound`]).
    fn frees_compound_child(&self) -> bool {
        let in_repr = match &self.repr {
            SetRepr::Raw(e) | SetRepr::Product(e) => e.iter().any(frees_compound),
            SetRepr::Subset(v) | SetRepr::Seq(v) => frees_compound(v),
            SetRepr::FcnSet(d, r) => frees_compound(d) || frees_compound(r),
            SetRepr::RcdSet(fs) => fs.iter().any(|(_, v)| frees_compound(v)),
            _ => false,
        };
        in_repr
            || self
                .elems
                .get()
                .is_some_and(|e| Arc::strong_count(e) == 1 && e.iter().any(frees_compound))
    }
}

impl FcnInner {
    /// Whether any child this function owns would be freed with it (see
    /// [`frees_compound`]).
    fn frees_compound_child(&self) -> bool {
        match &self.repr {
            FcnRepr::Tuple(e) => e.iter().any(frees_compound),
            FcnRepr::Record(fs) => fs.iter().any(|(_, v)| frees_compound(v)),
            FcnRepr::General(ps) => {
                ps.values.iter().any(frees_compound)
                    || (Arc::strong_count(&ps.domain) == 1
                        && ps.domain.keys.iter().any(frees_compound))
            }
        }
    }
}

// Dropping is iterative only where it must be. A value whose children would
// all survive it (scalars, or compound values shared elsewhere) drops its
// children in place. Otherwise the first `NATIVE_DROP_DEPTH` nested levels
// are freed by native recursion, and deeper ones through a work list, so
// nesting depth never becomes unbounded recursion depth. A child whose last
// other owner drops it concurrently is freed by that child's own `Drop`,
// which applies the same rules.
/// How many compound values one thread frees by native recursion before it
/// switches to the work list. Small enough that the recursion is cheap on any
/// worker stack; deep enough that ordinary values never need the work list.
const NATIVE_DROP_DEPTH: u32 = 32;

std::thread_local! {
    static DROP_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A level of bounded native recursion while dropping (see
/// [`NATIVE_DROP_DEPTH`]); none is available beyond the bound.
struct NativeDrop;

impl NativeDrop {
    fn enter() -> Option<Self> {
        let depth = DROP_DEPTH.get();
        (depth < NATIVE_DROP_DEPTH).then(|| {
            DROP_DEPTH.set(depth + 1);
            Self
        })
    }
}

impl Drop for NativeDrop {
    fn drop(&mut self) {
        DROP_DEPTH.set(DROP_DEPTH.get().saturating_sub(1));
    }
}

impl Drop for SetInner {
    fn drop(&mut self) {
        if !self.frees_compound_child() {
            return;
        }
        if let Some(_level) = NativeDrop::enter() {
            drop(std::mem::replace(&mut self.repr, SetRepr::Enum));
            drop(self.elems.take());
            return;
        }
        let mut stack = Vec::new();
        self.take_children(&mut stack);
        drain(stack);
    }
}

impl DomainInner {
    fn take_children(&mut self, out: &mut Vec<Value>) {
        for key in std::mem::take(&mut self.keys) {
            queue(out, key);
        }
    }
}

impl Drop for DomainInner {
    fn drop(&mut self) {
        if !self.keys.iter().any(frees_compound) {
            return;
        }
        if let Some(_level) = NativeDrop::enter() {
            drop(std::mem::take(&mut self.keys));
            return;
        }
        let mut stack = Vec::new();
        self.take_children(&mut stack);
        drain(stack);
    }
}

impl Drop for FcnInner {
    fn drop(&mut self) {
        if !self.frees_compound_child() {
            return;
        }
        if let Some(_level) = NativeDrop::enter() {
            drop(std::mem::replace(
                &mut self.repr,
                FcnRepr::Tuple(Box::default()),
            ));
            return;
        }
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

    /// Constructs a mathematical finite set in canonical value order.
    ///
    /// Unlike [`Self::set`], construction accepts mixed value shapes without
    /// reproducing TLC's partial sorting comparisons (ADR 0153). This does not
    /// change the semantics of direct scalar comparisons.
    ///
    /// ```
    /// use dive_value::{Ctx, Value};
    /// let ctx = Ctx::dive();
    /// let a = Value::set_canonical(vec![Value::Int(1), Value::TRUE], ctx)?;
    /// let b = Value::set_canonical(vec![Value::TRUE, Value::Int(1), Value::TRUE], ctx)?;
    /// assert_eq!(a, b);
    /// # Ok::<(), dive_value::ValueError>(())
    /// ```
    ///
    /// # Errors
    /// A symbolic child whose mathematical identity is not established, including
    /// inside a singleton, nested set, function key or function value, or allocation failure.
    pub fn set_canonical(elems: Vec<Self>, ctx: Ctx) -> Result<Self> {
        crate::construction::canonical_set(elems, ctx, &mut || true).map_err(|error| match error {
            crate::ConstructionError::Value(error) => error,
            other => ValueError::new(ErrorKind::DiveLimit, other.to_string()),
        })
    }

    /// Requires a complete explicit finite value tree, including raw set storage.
    ///
    /// Stored name order need not match `ctx`; this inspection does not sort or
    /// normalize values and never invokes a symbolic provider.
    ///
    /// Symbolic definitions are not mathematical inequality certificates.
    /// This checks singleton/nested sets and all function keys and values without
    /// enumerating a symbolic domain.
    ///
    /// ```
    /// use dive_value::{Ctx, Value};
    /// Value::Int(1).require_materialized(Ctx::dive())?;
    /// # Ok::<(), dive_value::ValueError>(())
    /// ```
    ///
    /// # Errors
    /// An unsupported symbolic child, or temporary allocation failure.
    pub fn require_materialized(&self, ctx: Ctx) -> Result<()> {
        crate::construction::require_closed(self, ctx, &mut || true).map_err(|error| match error {
            crate::ConstructionError::Value(error) => error,
            other => ValueError::new(ErrorKind::DiveLimit, other.to_string()),
        })
    }

    /// Produces a materialized operand for quiet extensional equality.
    ///
    /// Explicit and deferred results are checked recursively. Symbolic operator
    /// definitions are refused rather than compared structurally. As with value
    /// equality and hashing, provider evaluation here does not invoke host control.
    ///
    /// # Errors
    /// Provider evaluation errors, unsupported identity, or allocation failure.
    pub fn materialized_for_equality(&self, ctx: Ctx) -> Result<Self> {
        let _quiet = quiet::Quiet::enter();
        self.materialized_identity(ctx)
    }

    /// Materializes explicit/deferred children for an identity-demanding operation.
    ///
    /// Provider evaluation retains its normal controls. Symbolic operator
    /// definitions whose identity is unproved remain unsupported.
    ///
    /// # Errors
    /// Provider evaluation errors, unsupported identity, or allocation failure.
    pub fn materialized_identity(&self, ctx: Ctx) -> Result<Self> {
        crate::construction::materialized_identity(self, ctx, &mut || true, 0).map_err(|error| {
            match error {
                crate::ConstructionError::Value(error) => error,
                other => ValueError::new(ErrorKind::DiveLimit, other.to_string()),
            }
        })
    }

    /// Constructs an explicit set with cooperative normalization checkpoints.
    ///
    /// `proceed` returns false to interrupt. Closed materialized values use
    /// checked comparison, sorting and fallible temporary storage. Symbolic
    /// children retain the legacy constructor, whose enumeration is not yet
    /// cooperative. Final shared-value allocations retain Rust's allocation
    /// behavior; this is not an allocation-time memory ceiling.
    ///
    /// # Errors
    /// The original value error, interruption, or temporary allocation failure.
    ///
    /// ```
    /// use dive_value::{ConstructionError, Ctx, Value};
    /// let result = Value::set_controlled(vec![Value::Int(1)], Ctx::tlc(), &mut || false);
    /// assert!(matches!(result, Err(ConstructionError::Interrupted)));
    /// ```
    pub fn set_controlled(
        elems: Vec<Self>,
        ctx: Ctx,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Result<Self, crate::ConstructionError> {
        for value in &elems {
            if !crate::construction::closed(value, ctx, proceed)? {
                return Self::set(elems, ctx).map_err(Into::into);
            }
        }
        crate::construction::set(elems, ctx, proceed)
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

    /// Constructs a function with the controls and legacy symbolic fallback
    /// described by [`Self::set_controlled`].
    ///
    /// # Errors
    /// The original domain error, interruption, or temporary allocation failure.
    pub fn fcn_controlled(
        pairs: Vec<(Self, Self)>,
        ctx: Ctx,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Result<Self, crate::ConstructionError> {
        for (key, value) in &pairs {
            if !crate::construction::closed(key, ctx, proceed)?
                || !crate::construction::closed(value, ctx, proceed)?
            {
                return Self::fcn(pairs, ctx).map_err(Into::into);
            }
        }
        crate::construction::fcn(pairs, ctx, proceed)
    }

    /// Constructs a record with cooperative field sorting and fallible scratch
    /// storage. Final shared storage retains Rust's allocation behavior.
    ///
    /// # Errors
    /// The original duplicate-field error, interruption, or scratch allocation
    /// failure.
    pub fn record_controlled(
        fields: Vec<(Atom, Self)>,
        ctx: Ctx,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Result<Self, crate::ConstructionError> {
        crate::construction::record(fields, ctx, proceed)
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
