//! Immutable keys and conservative, provider-free indexing of general functions.

use std::slice;
use std::sync::{Arc, OnceLock};

use crate::{DomainLookup, Value, ValueOrder};

pub(crate) struct GeneralFcn {
    pub(crate) domain: Arc<DomainInner>,
    pub(crate) values: Box<[Value]>,
}

pub(crate) struct DomainInner {
    pub(crate) keys: Box<[Value]>,
    pub(crate) order: ValueOrder,
    comparable: OnceLock<bool>,
    index: OnceLock<DomainIndex>,
    tlc_order: OnceLock<Option<Box<[usize]>>>,
    canonical: OnceLock<bool>,
}

/// A certificate over the actual stored keys, never a certificate of TLC
/// normalization. `Unknown` preserves the original comparison path.
enum DomainIndex {
    Unknown,
    Integers {
        lo: i64,
        len: usize,
    },
    Product {
        lows: Box<[i64]>,
        widths: Box<[usize]>,
    },
}

impl GeneralFcn {
    pub(crate) fn new(pairs: Vec<(Value, Value)>, order: ValueOrder) -> Self {
        let (keys, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
        Self {
            domain: Arc::new(DomainInner {
                keys: keys.into_boxed_slice(),
                order,
                comparable: OnceLock::new(),
                index: OnceLock::new(),
                tlc_order: OnceLock::new(),
                canonical: OnceLock::new(),
            }),
            values: values.into_boxed_slice(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn iter(&self) -> std::iter::Zip<slice::Iter<'_, Value>, slice::Iter<'_, Value>> {
        self.domain.keys.iter().zip(self.values.iter())
    }

    pub(crate) fn get(&self, index: usize) -> Option<(&Value, &Value)> {
        self.domain.keys.get(index).zip(self.values.get(index))
    }
}

impl<'a> IntoIterator for &'a GeneralFcn {
    type Item = (&'a Value, &'a Value);
    type IntoIter = std::iter::Zip<slice::Iter<'a, Value>, slice::Iter<'a, Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl DomainInner {
    pub(crate) fn comparable(&self) -> bool {
        *self.comparable.get_or_init(|| {
            // Exactly the existing proof, with no provider evaluation. In
            // particular, a failed depth proof remains a conservative fallback.
            self.keys.len() <= 1 || Value::elements_comparable(&self.keys)
        })
    }

    /// Returns a known index/miss only for an exactly certified eager integer
    /// domain and an error-free compatible argument. Other arguments fall back.
    pub(crate) fn checked_index(&self, argument: &Value) -> DomainLookup {
        match self.index.get_or_init(|| DomainIndex::of(&self.keys)) {
            DomainIndex::Unknown => DomainLookup::Unknown,
            DomainIndex::Integers { lo, len } => {
                let Value::Int(value) = argument else {
                    return DomainLookup::Unknown;
                };
                let at = value.checked_sub(*lo).and_then(|v| usize::try_from(v).ok());
                at.filter(|&at| at < *len)
                    .map_or(DomainLookup::Missing, DomainLookup::Found)
            }
            DomainIndex::Product { lows, widths } => {
                let Some(tuple) = argument.as_fcn().and_then(crate::Fcn::as_tuple) else {
                    return DomainLookup::Unknown;
                };
                if tuple.len() != lows.len() {
                    return DomainLookup::Unknown;
                }
                let mut at = 0usize;
                let mut inside = true;
                for ((coordinate, &lo), &width) in tuple.iter().zip(lows).zip(widths) {
                    let Value::Int(value) = coordinate else {
                        return DomainLookup::Unknown;
                    };
                    // Even after a miss, inspect every coordinate for compatibility.
                    let offset = value.checked_sub(lo).and_then(|v| usize::try_from(v).ok());
                    if let Some(offset) = offset.filter(|&v| v < width) {
                        let Some(next) =
                            at.checked_mul(width).and_then(|at| at.checked_add(offset))
                        else {
                            return DomainLookup::Unknown;
                        };
                        at = next;
                    } else {
                        inside = false;
                    }
                }
                if inside {
                    DomainLookup::Found(at)
                } else {
                    DomainLookup::Missing
                }
            }
        }
    }

    pub(crate) fn integer_bounds(&self) -> Option<(i64, usize)> {
        match self.index.get_or_init(|| DomainIndex::of(&self.keys)) {
            DomainIndex::Integers { lo, len } => Some((*lo, *len)),
            _ => None,
        }
    }

    /// Only proven eager comparable keys may have a cached implementation view.
    /// Unknown nested sets keep the original sort on each use.
    pub(crate) fn tlc_order(&self) -> Option<&[usize]> {
        self.tlc_order
            .get_or_init(|| {
                if !self.comparable() || !self.keys.iter().all(crate::order::stable_key) {
                    return None;
                }
                let mut indices: Vec<usize> = (0..self.keys.len()).collect();
                indices.sort_by(|&a, &b| self.keys[a].cmp_with(&self.keys[b], ValueOrder::Tlc));
                Some(indices.into_boxed_slice())
            })
            .as_deref()
    }

    pub(crate) fn canonical(&self) -> bool {
        *self.canonical.get_or_init(|| {
            !self.keys.is_empty()
                && !self.keys.iter().all(|k| matches!(k, Value::Str(_)))
                && !self.keys.iter().enumerate().all(|(i, key)| {
                    i64::try_from(i)
                        .ok()
                        .and_then(|i| i.checked_add(1))
                        .is_some_and(|n| matches!(key, Value::Int(v) if *v == n))
                })
                && self.comparable()
                && self.keys.iter().all(crate::order::stable_key)
                && self
                    .keys
                    .windows(2)
                    .all(|w| w[0].cmp_with(&w[1], self.order).is_lt())
        })
    }

    pub(crate) fn matches_set(&self, set: &crate::Set) -> Option<bool> {
        use crate::value::SetRepr;
        match set.repr() {
            SetRepr::Interval(lo, hi) => {
                let (lo, hi) = (lo.to_i64()?, hi.to_i64()?);
                let (actual_lo, len) = self.integer_bounds()?;
                let actual_hi = actual_lo.checked_add(i64::try_from(len.checked_sub(1)?).ok()?)?;
                Some(lo == actual_lo && hi == actual_hi)
            }
            SetRepr::Product(factors) => {
                let DomainIndex::Product { lows, widths } =
                    self.index.get_or_init(|| DomainIndex::of(&self.keys))
                else {
                    return None;
                };
                if factors.len() != lows.len() {
                    return None;
                }
                let mut same = true;
                for ((factor, &low), &width) in factors.iter().zip(lows).zip(widths) {
                    let (lo, hi) = factor.as_set()?.as_interval()?;
                    let (lo, hi) = (lo.to_i64()?, hi.to_i64()?);
                    let actual_hi = low.checked_add(i64::try_from(width.checked_sub(1)?).ok()?)?;
                    same &= lo == low && hi == actual_hi;
                }
                Some(same)
            }
            _ => None,
        }
    }
}

impl DomainIndex {
    fn of(keys: &[Value]) -> Self {
        if let Some(Value::Int(lo)) = keys.first()
            && keys.iter().enumerate().all(|(i, key)| {
                i64::try_from(i)
                    .ok()
                    .and_then(|i| lo.checked_add(i))
                    .is_some_and(|value| matches!(key, Value::Int(v) if *v == value))
            })
        {
            return Self::Integers {
                lo: *lo,
                len: keys.len(),
            };
        }
        Self::product(keys).unwrap_or(Self::Unknown)
    }

    fn product(keys: &[Value]) -> Option<Self> {
        let first = keys.first()?.as_fcn()?.as_tuple()?;
        let mut lows: Vec<i64> = first
            .iter()
            .map(|v| match v {
                Value::Int(value) => Some(*value),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let mut highs = lows.clone();
        for key in keys {
            let tuple = key.as_fcn()?.as_tuple()?;
            if tuple.len() != lows.len() {
                return None;
            }
            for ((value, lo), hi) in tuple.iter().zip(&mut lows).zip(&mut highs) {
                let Value::Int(value) = value else {
                    return None;
                };
                *lo = (*lo).min(*value);
                *hi = (*hi).max(*value);
            }
        }
        let widths: Vec<usize> = lows
            .iter()
            .zip(highs)
            .map(|(&lo, hi)| usize::try_from(hi.checked_sub(lo)?.checked_add(1)?).ok())
            .collect::<Option<_>>()?;
        if widths.iter().try_fold(1usize, |n, &w| n.checked_mul(w))? != keys.len() {
            return None;
        }
        // This validates both completeness and the actual stored range mapping.
        // A mere cardinality or shape check would admit sparse/wrong-order data.
        for (expected, key) in keys.iter().enumerate() {
            let tuple = key.as_fcn()?.as_tuple()?;
            let mut at = 0usize;
            for ((value, &lo), &width) in tuple.iter().zip(&lows).zip(&widths) {
                let Value::Int(value) = value else {
                    return None;
                };
                let offset = usize::try_from(value.checked_sub(lo)?).ok()?;
                at = at.checked_mul(width)?.checked_add(offset)?;
            }
            if at != expected {
                return None;
            }
        }
        Some(Self::Product {
            lows: lows.into_boxed_slice(),
            widths: widths.into_boxed_slice(),
        })
    }
}

/// A borrowed function view; unknown cross-order keys keep the original owned
/// sort so evaluation/normalization effects occur at the original boundary.
pub(crate) enum OrderedFcn<'a> {
    Natural(&'a crate::Fcn),
    Indexed(&'a crate::Fcn, &'a [usize]),
    Owned(Vec<(Value, Value)>),
}

impl<'a> OrderedFcn<'a> {
    pub(crate) fn new(function: &'a crate::Fcn, order: ValueOrder) -> Self {
        if function.as_tuple().is_some() || function.order() == order {
            return Self::Natural(function);
        }
        if order == ValueOrder::Tlc
            && let crate::value::FcnRepr::General(general) = &function.0.repr
            && let Some(indices) = general.domain.tlc_order()
        {
            return Self::Indexed(function, indices);
        }
        let mut pairs: Vec<(Value, Value)> = function.iter().map(|(d, v)| (d, v.clone())).collect();
        pairs.sort_by(|a, b| a.0.cmp_with(&b.0, order));
        Self::Owned(pairs)
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Natural(f) | Self::Indexed(f, _) => f.len(),
            Self::Owned(pairs) => pairs.len(),
        }
    }

    pub(crate) fn entry(&self, index: usize) -> Option<(std::borrow::Cow<'_, Value>, &Value)> {
        match self {
            Self::Natural(f) => f.entry_cow(index),
            Self::Indexed(f, order) => order.get(index).and_then(|&i| f.entry_cow(i)),
            Self::Owned(pairs) => pairs
                .get(index)
                .map(|(d, v)| (std::borrow::Cow::Borrowed(d), v)),
        }
    }

    pub(crate) fn exact_domain_equals(&self, set: &crate::Set) -> Option<bool> {
        match self {
            Self::Natural(f) | Self::Indexed(f, _) => f.exact_domain_equals(set),
            Self::Owned(_) => None,
        }
    }

    pub(crate) fn value(&self, index: usize) -> &Value {
        match self {
            Self::Natural(f) | Self::Indexed(f, _) => {
                let at = match self {
                    Self::Indexed(_, order) => order[index],
                    _ => index,
                };
                match &f.0.repr {
                    crate::value::FcnRepr::Tuple(values) => &values[at],
                    crate::value::FcnRepr::Record(fields) => &fields[at].1,
                    crate::value::FcnRepr::General(general) => &general.values[at],
                }
            }
            Self::Owned(pairs) => &pairs[index].1,
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (std::borrow::Cow<'_, Value>, &Value)> {
        (0..self.len()).filter_map(|i| self.entry(i))
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "checked private invariants"
    )]
    use super::*;
    use crate::{Ctx, Fcn};

    fn coordinate(x: i64, y: i64) -> Value {
        Value::tuple(vec![Value::Int(x), Value::Int(y)])
    }

    #[test]
    fn exact_cartesian_index_rejects_sparse_duplicate_and_wrong_order_data() {
        let complete = [
            coordinate(0, 0),
            coordinate(0, 1),
            coordinate(1, 0),
            coordinate(1, 1),
        ];
        let index = DomainIndex::of(&complete);
        assert!(matches!(index, DomainIndex::Product { .. }));
        for keys in [
            vec![
                coordinate(0, 0),
                coordinate(0, 1),
                coordinate(1, 0),
                coordinate(2, 1),
            ],
            vec![
                coordinate(0, 0),
                coordinate(0, 1),
                coordinate(1, 0),
                coordinate(1, 0),
            ],
            vec![
                coordinate(0, 1),
                coordinate(0, 0),
                coordinate(1, 0),
                coordinate(1, 1),
            ],
            vec![coordinate(i64::MIN, 0), coordinate(i64::MAX, 0)],
            vec![Value::Int(i64::MIN), Value::Int(i64::MAX)],
        ] {
            assert!(matches!(DomainIndex::of(&keys), DomainIndex::Unknown));
        }
    }

    #[test]
    fn descriptor_reuse_keeps_range_and_fingerprint_independent() {
        for ctx in [Ctx::dive(), Ctx::tlc()] {
            let original = Fcn::general(
                vec![(Value::Int(0), Value::TRUE), (Value::Int(1), Value::FALSE)],
                ctx.order,
            );
            let changed = original
                .with_value_at(0, Value::FALSE, ctx)
                .expect("proved canonical domain");
            let crate::value::FcnRepr::General(before) = &original.0.repr else {
                unreachable!()
            };
            let crate::value::FcnRepr::General(after) = &changed.0.repr else {
                unreachable!()
            };
            assert!(Arc::ptr_eq(&before.domain, &after.domain));
            assert_eq!(original.apply(&Value::Int(0)), Some(&Value::TRUE));
            assert_eq!(changed.apply(&Value::Int(0)), Some(&Value::FALSE));
            assert!(!original.ptr_eq(&changed));
            assert!(changed.with_value_at(2, Value::TRUE, ctx).is_err());
        }
    }

    #[test]
    fn domain_drops_deep_keys_when_last_owner_is_outside_a_function() {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(|| {
                let mut key = Value::TRUE;
                for _ in 0..100_000 {
                    // Eager child hashes keep construction bottom-up and stack-safe.
                    key = Value::Fcn(Fcn::general(
                        vec![(key, Value::FALSE)],
                        ValueOrder::Canonical,
                    ));
                }
                let general = GeneralFcn::new(vec![(key, Value::TRUE)], ValueOrder::Canonical);
                let retained = Arc::clone(&general.domain);
                drop(general);
                let share = |value: Value| {
                    let mut function = Fcn::general(
                        retained
                            .keys
                            .iter()
                            .map(|key| (key.clone(), value.clone()))
                            .collect(),
                        ValueOrder::Canonical,
                    );
                    let inner = Arc::get_mut(&mut function.0).expect("new function uniquely owned");
                    let crate::value::FcnRepr::General(general) = &mut inner.repr else {
                        unreachable!()
                    };
                    general.domain = Arc::clone(&retained);
                    function
                };
                let first = share(Value::TRUE);
                let second = share(Value::FALSE);
                drop(first);
                drop(retained);
                drop(second);
                let mut key = Value::FALSE;
                for _ in 0..100_000 {
                    key = Value::Fcn(Fcn::general(
                        vec![(key, Value::TRUE)],
                        ValueOrder::Canonical,
                    ));
                }
                let construction: Result<(), ()> = {
                    let _pending =
                        GeneralFcn::new(vec![(key, Value::FALSE)], ValueOrder::Canonical);
                    Err(())
                };
                assert!(construction.is_err());
            })
            .expect("thread")
            .join()
            .expect("bounded Domain drop");
    }
}
