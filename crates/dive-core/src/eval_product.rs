//! Receiver-preserving Product scalar operations. Source: tlaplus value
//! isFinite/isEmpty/size methods and Nat/Int overrides, 94d0c50.

use super::Machine;
use crate::node::Kind;
use crate::{Error, Handle};

fn refused() -> Error {
    Error::unsupported("Cardinality/IsFiniteSet of a Product with an unproved factor shape or size")
}

fn bounded(n: u64) -> Result<u64, Error> {
    if i32::try_from(n).is_ok() {
        Ok(n)
    } else {
        Err(refused())
    }
}

impl Machine {
    pub(super) fn product_finiteness(&mut self, h: Handle) -> Result<bool, Error> {
        self.factor_finiteness(h, 0)
    }

    /// Source Boolean demand, distinct from complete finite size admission.
    /// Known infinite factors never turn an unknown operand into a false answer.
    fn factor_finiteness(&mut self, h: Handle, depth: u8) -> Result<bool, Error> {
        self.charge(1)?;
        if depth >= 64 || !h.is_compound() || self.heap.is_lazy(h) {
            return Err(refused());
        }
        let view = self.heap.view(h);
        let (kind, len) = (view.kind, view.data.len());
        match kind {
            Kind::Nat | Kind::Int if len == 0 => Ok(false),
            Kind::Subset if len == 1 => {
                let base = view.data[0];
                self.factor_finiteness(base, depth + 1)
            }
            Kind::Product | Kind::RecSet
                if (kind == Kind::Product && len >= 2)
                    || (kind == Kind::RecSet && len.is_multiple_of(2)) =>
            {
                let mut all_finite = true;
                let start = if kind == Kind::RecSet { len / 2 } else { 0 };
                for index in start..len {
                    let child = self.heap.view(h).data[index];
                    if self.factor_finiteness(child, depth + 1)? {
                        if self.finite_empty(child, depth + 1)? {
                            return Ok(true);
                        }
                    } else {
                        all_finite = false;
                    }
                }
                Ok(all_finite)
            }
            Kind::FcnSet if len == 2 => {
                let (domain, range) = (view.data[0], view.data[1]);
                let finite_domain = self.factor_finiteness(domain, depth + 1)?;
                if finite_domain && self.finite_empty(domain, depth + 1)? {
                    return Ok(true);
                }
                let finite_range = self.factor_finiteness(range, depth + 1)?;
                if finite_range && self.finite_empty(range, depth + 1)? {
                    return Ok(true);
                }
                if finite_domain && finite_range {
                    return Ok(true);
                }
                Ok(finite_range && self.finite_size(range, depth + 1)? == 1)
            }
            _ if self.finite_factor(h, depth)? => Ok(true),
            _ => Err(refused()),
        }
    }

    pub(super) fn finite_product(&mut self, h: Handle) -> Result<(), Error> {
        if self.finite_factor(h, 0)? {
            Ok(())
        } else {
            Err(refused())
        }
    }

    /// Complete read-only admission: every constructor operand must itself be
    /// admitted. No scalar, infinite, deferred or unknown operand is guessed.
    fn finite_factor(&mut self, h: Handle, depth: u8) -> Result<bool, Error> {
        self.charge(1)?;
        if depth >= 64 || !h.is_compound() || self.heap.is_lazy(h) {
            return Ok(false);
        }
        let view = self.heap.view(h);
        let (kind, lo, len) = (view.kind, view.lo, view.data.len());
        match kind {
            Kind::Set => Ok(!self.heap.has_product_receiver(h)?),
            Kind::SetEnum => {
                Ok(self.heap.owns_local_literal(h) && !self.heap.has_product_receiver(h)?)
            }
            Kind::Interval => Ok(view
                .data
                .first()
                .and_then(|v| v.as_int())
                .is_some_and(|hi| {
                    hi < lo
                        || hi
                            .checked_sub(lo)
                            .and_then(|n| n.checked_add(1))
                            .is_some_and(|n| n <= i64::from(i32::MAX))
                })),
            Kind::Subset | Kind::FcnSet | Kind::Product | Kind::RecSet => {
                let valid = match kind {
                    Kind::Subset => len == 1,
                    Kind::FcnSet => len == 2,
                    Kind::Product => len >= 2,
                    Kind::RecSet => len.is_multiple_of(2),
                    _ => false,
                };
                if !valid {
                    return Ok(false);
                }
                let start = if kind == Kind::RecSet { len / 2 } else { 0 };
                for index in start..len {
                    let child = self.heap.view(h).data[index];
                    if !self.finite_factor(child, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Source emptiness for known shapes, refusing any demanded unknown shape.
    /// Explicit storage is counted without normalization; short circuits match TLC.
    fn finite_empty(&mut self, h: Handle, depth: u8) -> Result<bool, Error> {
        self.charge(1)?;
        if depth >= 64 {
            return Err(refused());
        }
        let view = self.heap.view(h);
        let (kind, lo, len) = (view.kind, view.lo, view.data.len());
        match kind {
            Kind::Nat | Kind::Int if len == 0 => Ok(false),
            Kind::Set | Kind::SetEnum => Ok(len == 0),
            Kind::Interval => Ok(view
                .data
                .first()
                .and_then(|v| v.as_int())
                .ok_or_else(refused)?
                < lo),
            Kind::Subset => Ok(false),
            Kind::FcnSet => {
                let (domain, range) = (view.data[0], view.data[1]);
                Ok(
                    !self.finite_empty(domain, depth + 1)?
                        && self.finite_empty(range, depth + 1)?,
                )
            }
            Kind::Product | Kind::RecSet => {
                let start = if kind == Kind::RecSet { len / 2 } else { 0 };
                for index in start..len {
                    let child = self.heap.view(h).data[index];
                    if self.finite_empty(child, depth + 1)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Err(refused()),
        }
    }

    pub(super) fn product_cardinality(&mut self, h: Handle) -> Result<Handle, Error> {
        self.finite_product(h)?;
        let size = self.finite_size(h, 0)?;
        Handle::int(i64::try_from(size).map_err(|_| refused())?).ok_or_else(refused)
    }

    /// Source `size()` demand order, retaining the original Product. Only actual
    /// explicit-set size demands normalize owned axes; no Product is interned.
    fn finite_size(&mut self, h: Handle, depth: u8) -> Result<u64, Error> {
        self.charge(1)?;
        if depth >= 64 {
            return Err(refused());
        }
        let view = self.heap.view(h);
        let (kind, len) = (view.kind, view.data.len());
        match kind {
            Kind::Set | Kind::SetEnum | Kind::Interval => bounded(self.work_cardinality(h)?),
            Kind::Subset => {
                let base = view.data[0];
                let size = self.finite_size(base, depth + 1)?;
                if size >= 31 {
                    return Err(refused());
                }
                Ok(1u64 << size)
            }
            Kind::FcnSet => {
                let (domain, range) = (view.data[0], view.data[1]);
                if self.finite_empty(domain, depth + 1)? {
                    return Ok(1);
                }
                if self.finite_empty(range, depth + 1)? {
                    return Ok(0);
                }
                if self.finite_size(range, depth + 1)? == 1 {
                    return Ok(1);
                }
                let domain_size = self.finite_size(domain, depth + 1)?;
                let range_size = self.finite_size(range, depth + 1)?;
                let power = u32::try_from(domain_size).map_err(|_| refused())?;
                bounded(range_size.checked_pow(power).ok_or_else(refused)?)
            }
            Kind::Product | Kind::RecSet => {
                if self.finite_empty(h, depth)? {
                    return Ok(0);
                }
                let mut size = 1u64;
                let start = if kind == Kind::RecSet { len / 2 } else { 0 };
                for index in start..len {
                    let child = self.heap.view(h).data[index];
                    let child_size = self.finite_size(child, depth + 1)?;
                    size = bounded(size.checked_mul(child_size).ok_or_else(refused)?)?;
                }
                Ok(size)
            }
            _ => Err(refused()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "closed test values")]
    use super::*;
    use crate::{Opts, Semantics, Store, memo::Memo};
    use std::sync::Arc;

    #[test]
    fn scalar_methods_keep_receivers_and_only_normalize_demanded_axes() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let one = Handle::int(1).unwrap();
            let raw = m.heap.local_literal(&[one, zero, one]).unwrap();
            let axis = m.heap.interval(0, 0);
            let product = m.heap.symbolic(Kind::Product, 0, &[raw, axis]);
            m.finite_product(product).unwrap();
            assert!(m.heap.has_raw_construction(raw));
            assert_eq!(m.product_cardinality(product).unwrap().as_int(), Some(2));
            assert!(!m.heap.has_raw_construction(raw));
            assert_eq!(m.heap.kind(product), Some(Kind::Product));
            assert!(m.member(zero, product).is_err());
            let mixed = m.heap.local_literal(&[zero, Handle::TRUE]).unwrap();
            let empty = m.heap.empty_set();
            let functions = m.heap.symbolic(Kind::FcnSet, 0, &[empty, mixed]);
            let skipped = m.heap.symbolic(Kind::Product, 0, &[functions, axis]);
            assert_eq!(m.product_cardinality(skipped).unwrap().as_int(), Some(1));
            assert!(m.heap.has_raw_construction(mixed));
            let demanded = m.heap.symbolic(Kind::Product, 0, &[mixed, axis]);
            assert!(m.product_cardinality(demanded).is_err());
            assert!(!m.heap.poisoned());
        }
    }

    #[test]
    fn incomplete_factor_trees_and_exhausted_work_never_answer() {
        let mut m = Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            Memo::new(&[]),
        );
        let zero = Handle::int(0).unwrap();
        let axis = m.heap.interval(0, 1);
        let invalid = m.heap.symbolic(Kind::Product, 0, &[axis, zero]);
        assert!(m.finite_product(invalid).is_err());
        let valid = m.heap.symbolic(Kind::Product, 0, &[axis, axis]);
        m.set_fuel(0);
        assert!(m.finite_product(valid).is_err());
        assert!(m.product_cardinality(valid).is_err());
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn boolean_finiteness_keeps_unknown_demand_and_raw_receiver_authority() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let raw = m.heap.local_literal(&[zero, Handle::TRUE]).unwrap();
            let empty = m.heap.empty_set();
            for kind in [Kind::Nat, Kind::Int] {
                let infinite = m.heap.symbolic(kind, 0, &[]);
                for axes in [[infinite, raw], [raw, infinite]] {
                    let product = m.heap.symbolic(Kind::Product, 0, &axes);
                    assert!(!m.product_finiteness(product).unwrap());
                    assert_eq!(m.heap.kind(product), Some(Kind::Product));
                    assert!(m.heap.has_raw_construction(raw));
                    assert!(m.product_cardinality(product).is_err());
                }
                let nested = m.heap.symbolic(Kind::Product, 0, &[infinite, empty]);
                let outer = m.heap.symbolic(Kind::Product, 0, &[nested, raw]);
                assert!(m.product_finiteness(outer).unwrap());
                for axes in [[infinite, zero], [zero, infinite]] {
                    let invalid = m.heap.symbolic(Kind::Product, 0, &axes);
                    assert!(m.product_finiteness(invalid).is_err());
                }
                let invalid = m.heap.symbolic(kind, 0, &[zero]);
                let product = m.heap.symbolic(Kind::Product, 0, &[raw, invalid]);
                assert!(m.product_finiteness(product).is_err());
            }
            assert!(!m.heap.poisoned());
        }
    }
}
