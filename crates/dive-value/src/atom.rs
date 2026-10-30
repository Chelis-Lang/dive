//! Interned names: strings, model values and record field names.
//!
//! TLC keeps every name in one global table (`util.UniqueString`) and orders names by
//! their position in it (`UniqueString.compareTo` returns `this.tok - other.tok`).
//! dive mirrors that with an [`Interner`] that hands out [`Atom`]s numbered in
//! insertion order. Fed in SANY/TLC's encounter order, the numbering reproduces TLC's
//! order of strings, model values and record fields; see ADR 0012.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use crate::ctx::ValueOrder;

impl crate::Value {
    /// Requested heap payload retained by a narrow immutable conversion cache.
    ///
    /// Admits only booleans, inline integers and flat tuples/records containing
    /// those values. The caller separately charges its table capacity, including
    /// inline Value/Atom owners. Record name payload is shared only when its Arc
    /// is already pinned by `names`; foreign owners and all other shapes decline.
    /// Includes private function headers (and fixed fingerprint cache) and boxed
    /// element/field storage. Opaque Arc headers and allocator overhead are not
    /// measured; this is not a whole-process memory allowance.
    /// `proceed` checks work before each component; false declines caching.
    ///
    /// ```
    /// use dive_value::{Ctx, Interner, Value};
    /// let names = Interner::new();
    /// let record = Value::record(vec![(names.intern("x"), Value::Int(1))], Ctx::dive())?;
    /// assert!(record.flat_cache_heap_bytes(&names, &mut || true).is_some());
    /// assert_eq!(record.flat_cache_heap_bytes(&names, &mut || false), None);
    /// # Ok::<(), dive_value::ValueError>(())
    /// ```
    #[must_use]
    pub fn flat_cache_heap_bytes(
        &self,
        names: &Interner,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Option<usize> {
        use crate::value::{FcnInner, FcnRepr};
        if !proceed() {
            return None;
        }
        let immediate = |value: &Self| matches!(value, Self::Bool(_) | Self::Int(_));
        match self {
            Self::Bool(_) | Self::Int(_) => Some(0),
            Self::Fcn(function) => {
                let payload = match &function.0.repr {
                    FcnRepr::Tuple(elements) => {
                        for value in elements {
                            if !proceed() || !immediate(value) {
                                return None;
                            }
                        }
                        std::mem::size_of_val(elements.as_ref())
                    }
                    FcnRepr::Record(fields) => {
                        for (name, value) in fields {
                            if !proceed()
                                || !immediate(value)
                                || !names
                                    .resolve(name.tok())
                                    .is_some_and(|known| Arc::ptr_eq(&name.0, &known.0))
                            {
                                return None;
                            }
                        }
                        std::mem::size_of_val(fields.as_ref())
                    }
                    FcnRepr::General(_) => return None,
                };
                std::mem::size_of::<FcnInner>().checked_add(payload)
            }
            _ => None,
        }
    }

    /// Conservative requested payload for a closed function-tree read cache.
    ///
    /// Admits Bool/Int, strings pinned by this exact `names` interner, and
    /// recursively closed tuples, records and functions. General function keys
    /// must be Bool/Int or run-owned strings. All sets (including normalized
    /// sets), model values, big integers and other shapes decline without
    /// evaluation or normalization. Function literal classes remain unchanged.
    ///
    /// Counts each occurrence, conservatively overcharging shared children.
    /// Includes function headers, boxed arrays, general-domain headers and the
    /// maximum future scalar-domain TLC-order array. Scalar keys cannot create
    /// the allocating product-domain index. Fingerprint and scalar lookup caches
    /// are fixed header fields and have no normalization-history side effects.
    /// The caller separately charges its table and row array. Opaque Arc headers
    /// and allocator overhead remain excluded; this is not a process memory cap.
    /// Inspection checks `proceed` at each step and declines after 4,096 values
    /// or 64 function levels. Declining only bypasses caching.
    ///
    /// ```
    /// use dive_value::{Ctx, Interner, Value};
    /// let names = Interner::new();
    /// let nested = Value::fcn(vec![(Value::Int(0),
    ///     Value::tuple(vec![Value::string(&names, "tok")]))], Ctx::tlc())?;
    /// assert!(nested.closed_function_cache_heap_bytes(&names, &mut || true).is_some());
    /// # Ok::<(), dive_value::ValueError>(())
    /// ```
    #[must_use]
    pub fn closed_function_cache_heap_bytes(
        &self,
        names: &Interner,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Option<usize> {
        closed_cache_bytes(self, names, &mut 4096, 0, proceed)
    }
}

fn run_owned(name: &Atom, names: &Interner) -> bool {
    names
        .resolve(name.tok())
        .is_some_and(|known| Arc::ptr_eq(&name.0, &known.0))
}

fn closed_cache_bytes(
    value: &crate::Value,
    names: &Interner,
    remaining: &mut usize,
    depth: usize,
    proceed: &mut dyn FnMut() -> bool,
) -> Option<usize> {
    use crate::Value;
    use crate::function_domain::DomainInner;
    use crate::value::{FcnInner, FcnRepr};
    *remaining = remaining.checked_sub(1)?;
    if !proceed() {
        return None;
    }
    match value {
        Value::Bool(_) | Value::Int(_) => Some(0),
        Value::Str(name) if run_owned(name, names) => Some(0),
        Value::Fcn(function) if depth < 64 => {
            let mut bytes = std::mem::size_of::<FcnInner>();
            match &function.0.repr {
                FcnRepr::Tuple(elements) => {
                    bytes = bytes.checked_add(std::mem::size_of_val(elements.as_ref()))?;
                    for child in elements {
                        bytes = bytes.checked_add(closed_cache_bytes(
                            child,
                            names,
                            remaining,
                            depth + 1,
                            proceed,
                        )?)?;
                    }
                }
                FcnRepr::Record(fields) => {
                    bytes = bytes.checked_add(std::mem::size_of_val(fields.as_ref()))?;
                    for (name, child) in fields {
                        if !proceed() || !run_owned(name, names) {
                            return None;
                        }
                        bytes = bytes.checked_add(closed_cache_bytes(
                            child,
                            names,
                            remaining,
                            depth + 1,
                            proceed,
                        )?)?;
                    }
                }
                FcnRepr::General(general) => {
                    bytes = bytes.checked_add(std::mem::size_of::<DomainInner>())?;
                    bytes =
                        bytes.checked_add(std::mem::size_of_val(general.domain.keys.as_ref()))?;
                    bytes = bytes.checked_add(std::mem::size_of_val(general.values.as_ref()))?;
                    // DomainIndex::of can only choose Integers or Unknown for
                    // scalar keys: Product requires a tuple first key. The sole
                    // later retained allocation is tlc_order's boxed usize[n].
                    bytes = bytes.checked_add(
                        general
                            .domain
                            .keys
                            .len()
                            .checked_mul(std::mem::size_of::<usize>())?,
                    )?;
                    for key in &general.domain.keys {
                        if !matches!(key, Value::Bool(_) | Value::Int(_) | Value::Str(_)) {
                            return None;
                        }
                        bytes = bytes.checked_add(closed_cache_bytes(
                            key,
                            names,
                            remaining,
                            depth + 1,
                            proceed,
                        )?)?;
                    }
                    for child in &general.values {
                        bytes = bytes.checked_add(closed_cache_bytes(
                            child,
                            names,
                            remaining,
                            depth + 1,
                            proceed,
                        )?)?;
                    }
                }
            }
            Some(bytes)
        }
        _ => None,
    }
}

/// An interned name. Cloning is a reference-count increment.
///
/// Equality is by text, so atoms from different interners still compare correctly for
/// equality; ordering by token is only meaningful within one interner.
#[derive(Clone)]
pub struct Atom(Arc<AtomInner>);

struct AtomInner {
    text: Box<str>,
    tok: u32,
    hash: u64,
    utf16_len: u32,
    /// TLC's model-value type: the first UTF-16 unit when the name has more than two
    /// units and its second unit is `_` (`ModelValue` constructor). 0 means untyped.
    mv_type: u16,
}

impl Atom {
    fn new(text: &str, tok: u32) -> Self {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mv_type = if units.len() > 2 && units[1] == u16::from(b'_') {
            units[0]
        } else {
            0
        };
        Self(Arc::new(AtomInner {
            text: text.into(),
            tok,
            hash: text_hash(text),
            utf16_len: u32::try_from(units.len()).unwrap_or(u32::MAX),
            mv_type,
        }))
    }

    /// The name's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0.text
    }

    /// The intern token: the position at which the name was first interned.
    #[must_use]
    pub fn tok(&self) -> u32 {
        self.0.tok
    }

    /// The length in UTF-16 code units (Java's `String.length()`).
    #[must_use]
    pub fn utf16_len(&self) -> u32 {
        self.0.utf16_len
    }

    /// TLC's model-value type character, as a UTF-16 unit, if the name is typed
    /// (`x_name`: more than two units, the second being `_`).
    #[must_use]
    pub fn mv_type(&self) -> Option<u16> {
        (self.0.mv_type != 0).then_some(self.0.mv_type)
    }

    /// A precomputed, deterministic 64-bit hash of the text.
    #[must_use]
    pub fn text_hash(&self) -> u64 {
        self.0.hash
    }

    /// Orders two atoms under the given string order.
    #[must_use]
    pub fn cmp_with(&self, other: &Self, order: ValueOrder) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            return Ordering::Equal;
        }
        match order {
            ValueOrder::Tlc => self
                .0
                .tok
                .cmp(&other.0.tok)
                .then_with(|| self.0.text.cmp(&other.0.text)),
            ValueOrder::Canonical => self.0.text.cmp(&other.0.text),
        }
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.hash == other.0.hash && self.0.text == other.0.text)
    }
}

impl Eq for Atom {}

impl Hash for Atom {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash);
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}#{}", self.as_str(), self.tok())
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// FNV-1a over the bytes, then a 64-bit finalizer. Deterministic across runs.
pub(crate) fn text_hash(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    mix64(h)
}

/// The `splitmix64` finalizer.
pub(crate) const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The table of names for one run. Thread-safe; share it with an [`Arc`].
///
/// Tokens are assigned in insertion order, starting at 0. To reproduce TLC's order of
/// strings, model values and record fields, intern names in the order SANY and TLC
/// first meet them (ADR 0012).
///
/// ```
/// use dive_value::Interner;
/// let names = Interner::new();
/// let b = names.intern("b");
/// let a = names.intern("a");
/// assert!(b.tok() < a.tok());
/// assert_eq!(names.intern("b"), b);
/// ```
#[derive(Default)]
pub struct Interner {
    table: Mutex<Table>,
}

#[derive(Default)]
struct Table {
    by_text: HashMap<Box<str>, Atom>,
    /// `by_tok[t]` is the atom with token `t`.
    by_tok: Vec<Atom>,
}

impl Interner {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the atom for `text`, interning it with the next token if it is new.
    pub fn intern(&self, text: &str) -> Atom {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(atom) = table.by_text.get(text) {
            return atom.clone();
        }
        let tok = u32::try_from(table.by_tok.len()).unwrap_or(u32::MAX);
        let atom = Atom::new(text, tok);
        table.by_text.insert(text.into(), atom.clone());
        table.by_tok.push(atom.clone());
        atom
    }

    /// Returns the atom with token `tok`, if one has been interned.
    ///
    /// This is the inverse of [`Atom::tok`] for this table. State encodings store
    /// names by token ([`crate::codec`]), and decoding resolves them here.
    ///
    /// ```
    /// use dive_value::Interner;
    /// let names = Interner::new();
    /// let a = names.intern("a");
    /// assert_eq!(names.resolve(a.tok()), Some(a));
    /// assert_eq!(names.resolve(7), None);
    /// ```
    #[must_use]
    pub fn resolve(&self, tok: u32) -> Option<Atom> {
        let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        usize::try_from(tok)
            .ok()
            .and_then(|t| table.by_tok.get(t))
            .cloned()
    }

    /// Returns the atom for `text` if it has been interned.
    #[must_use]
    pub fn get(&self, text: &str) -> Option<Atom> {
        let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.by_text.get(text).cloned()
    }

    /// The number of interned names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_tok
            .len()
    }

    /// Whether no name has been interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for Interner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner")
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "flat-cache storage and ownership fixtures"
)]
mod flat_cache_tests {
    use super::*;
    use crate::{Ctx, Value};

    #[test]
    fn only_flat_immediates_with_run_owned_names_receive_known_payload_charge() {
        let names = Interner::new();
        let key = names.intern("x");
        let record =
            Value::record(vec![(key.clone(), Value::Int(1))], Ctx::dive()).expect("record");
        let general =
            Value::fcn(vec![(Value::Str(key), Value::Int(1))], Ctx::dive()).expect("function");
        let expected =
            std::mem::size_of::<crate::value::FcnInner>() + std::mem::size_of::<(Atom, Value)>();
        assert_eq!(
            record.flat_cache_heap_bytes(&names, &mut || true),
            Some(expected)
        );
        assert_eq!(
            general.flat_cache_heap_bytes(&names, &mut || true),
            Some(expected)
        );
        let tuple = Value::tuple(vec![Value::Int(1), Value::Bool(true)]);
        assert_eq!(
            tuple.flat_cache_heap_bytes(&names, &mut || true),
            Some(std::mem::size_of::<crate::value::FcnInner>() + 2 * std::mem::size_of::<Value>())
        );
        let foreign = Interner::new();
        let other = Value::record(vec![(foreign.intern("x"), Value::Int(1))], Ctx::dive())
            .expect("foreign record");
        assert_eq!(other.flat_cache_heap_bytes(&names, &mut || true), None);
        for unsupported in [
            Value::tuple(vec![tuple]),
            Value::string(&names, "x"),
            Value::empty_set(Ctx::dive()),
            Value::fcn(vec![(Value::Int(0), Value::Int(1))], Ctx::dive()).expect("general domain"),
        ] {
            assert_eq!(
                unsupported.flat_cache_heap_bytes(&names, &mut || true),
                None
            );
        }
        assert_eq!(record.flat_cache_heap_bytes(&names, &mut || false), None);
        assert_eq!(
            Value::Int(1).flat_cache_heap_bytes(&names, &mut || true),
            Some(0)
        );
    }

    #[test]
    fn closed_function_charge_covers_nested_arrays_and_future_domain_order() {
        use crate::function_domain::DomainInner;
        use crate::value::{FcnInner, FcnRepr};
        for ctx in [Ctx::dive(), Ctx::tlc()] {
            let names = Interner::new();
            let child = Value::record(
                vec![(
                    names.intern("message"),
                    Value::tuple(vec![Value::string(&names, "tok"), Value::Int(3)]),
                )],
                ctx,
            )
            .expect("record");
            let child_bytes = child
                .closed_function_cache_heap_bytes(&names, &mut || true)
                .expect("closed child");
            let value = Value::fcn(
                vec![(Value::Int(0), child.clone()), (Value::Int(1), child)],
                ctx,
            )
            .expect("integer-domain function");
            let expected = std::mem::size_of::<FcnInner>()
                + std::mem::size_of::<DomainInner>()
                + 4 * std::mem::size_of::<Value>()
                + 2 * std::mem::size_of::<usize>()
                + 2 * child_bytes;
            assert_eq!(
                value.closed_function_cache_heap_bytes(&names, &mut || true),
                Some(expected)
            );
            let repr = &value.as_fcn().expect("function").0.repr;
            assert!(matches!(repr, FcnRepr::General(_)));
            if let FcnRepr::General(general) = repr {
                assert_eq!(
                    general.domain.checked_index(&Value::Int(1)),
                    crate::DomainLookup::Found(1)
                );
                assert_eq!(general.domain.tlc_order().expect("pure order").len(), 2);
            }
            assert_eq!(
                value.closed_function_cache_heap_bytes(&names, &mut || true),
                Some(expected),
                "lazy header facts/order allocation were prepaid"
            );
        }
    }

    #[test]
    fn closed_inspection_declines_history_foreign_names_and_product_keys_without_effects() {
        let names = Interner::new();
        let foreign = Interner::new();
        for leaf in [
            Value::empty_set(Ctx::dive()),
            Value::set_raw(vec![Value::Int(1)], Ctx::dive()),
            Value::string(&foreign, "foreign"),
            Value::model(&names, "model"),
            Value::int(crate::Int::from_big(num_bigint::BigInt::from(1u64) << 80)),
        ] {
            let tree = Value::tuple(vec![leaf]);
            assert_eq!(
                tree.closed_function_cache_heap_bytes(&names, &mut || true),
                None
            );
        }
        let key = Value::tuple(vec![Value::Int(0)]);
        let product =
            Value::fcn(vec![(key, Value::Int(1))], Ctx::dive()).expect("product-domain function");
        assert_eq!(
            product.closed_function_cache_heap_bytes(&names, &mut || true),
            None
        );
        let value = Value::tuple(vec![Value::Int(1)]);
        assert_eq!(
            value.closed_function_cache_heap_bytes(&names, &mut || false),
            None
        );
        let mut deep = Value::Int(0);
        for _ in 0..65 {
            deep = Value::tuple(vec![deep]);
        }
        assert_eq!(
            deep.closed_function_cache_heap_bytes(&names, &mut || true),
            None
        );
        let wide = Value::tuple(vec![Value::Int(0); 4096]);
        assert_eq!(
            wide.closed_function_cache_heap_bytes(&names, &mut || true),
            None
        );
        assert!(
            Value::tuple(vec![Value::Int(0); 4095])
                .closed_function_cache_heap_bytes(&names, &mut || true)
                .is_some()
        );
    }
}
