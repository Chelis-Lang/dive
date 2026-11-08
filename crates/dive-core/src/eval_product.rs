//! Bounded symbolic finiteness and Product cardinality. Source: tlaplus value
//! isFinite/isEmpty/size methods, Sequences.isFinite and Nat/Int overrides,
//! 94d0c50.

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
    /// Whether an empty factor may decide finiteness before later factors
    /// are proved. TLC's `isFinite` stops at the first empty factor it meets,
    /// so a later non-set factor answers TRUE or fails by encounter order.
    /// Product and function-set operands are met in source order in both
    /// profiles, the order TLC uses, and the goldens pin those answers
    /// (`IsFiniteSet({} \X 0)` is TRUE). Record fields are met in TLC's
    /// intern order only under token string order; canonical field order
    /// would make `[a : {}, b : 0]` answer TRUE where TLC may fail, so there
    /// every field is proved first and an unproved one refuses.
    fn source_factor_order(&self, kind: Kind) -> bool {
        kind != Kind::RecSet
            || self.heap.store().semantics().string_order == crate::strings::StringOrder::Token
    }

    pub(super) fn symbolic_finiteness(&mut self, h: Handle) -> Result<bool, Error> {
        if matches!(
            self.heap.kind(h),
            Some(
                Kind::Product
                    | Kind::Subset
                    | Kind::FcnSet
                    | Kind::RecSet
                    | Kind::SeqSet
                    | Kind::Union
            )
        ) {
            self.factor_finiteness(h, 0)
        } else {
            self.charge(1)?;
            Ok(self.heap.is_finite(h))
        }
    }

    /// Mathematical Boolean finiteness, distinct from complete finite size admission.
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
            Kind::Union if len == 2 => {
                let left = view.data[0];
                let right = view.data[1];
                let left_finite = self.factor_finiteness(left, depth + 1)?;
                let right_finite = self.factor_finiteness(right, depth + 1)?;
                Ok(left_finite && right_finite)
            }
            Kind::SeqSet if len == 1 => {
                let range = view.data[0];
                if !self.factor_finiteness(range, depth + 1)? {
                    return Ok(false);
                }
                self.finite_empty(range, depth + 1)
            }
            Kind::Product | Kind::RecSet
                if (kind == Kind::Product && len >= 2)
                    || (kind == Kind::RecSet && len.is_multiple_of(2)) =>
            {
                let tlc_order = self.source_factor_order(kind);
                let (mut all_finite, mut any_empty) = (true, false);
                let start = if kind == Kind::RecSet { len / 2 } else { 0 };
                for index in start..len {
                    let child = self.heap.view(h).data[index];
                    if self.factor_finiteness(child, depth + 1)? {
                        if !any_empty && self.finite_empty(child, depth + 1)? {
                            if tlc_order {
                                return Ok(true);
                            }
                            any_empty = true;
                        }
                    } else {
                        all_finite = false;
                    }
                }
                Ok(any_empty || all_finite)
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
            Kind::Set | Kind::SetEnum => Ok(true),
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
            Kind::Subset | Kind::SeqSet => Ok(false),
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
    fn sequence_finiteness_keeps_empty_infinite_unknown_and_stop_cases() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let raw = m.heap.set_from(&mut vec![zero, Handle::TRUE]);
            let empty = m.heap.interval(1, 0);
            let product = m.heap.symbolic(Kind::Product, 0, &[empty, raw]);
            let sequences = m.heap.symbolic(Kind::SeqSet, 0, &[product]);
            assert_eq!(m.symbolic_finiteness(sequences), Ok(true));

            assert_eq!(m.heap.view(sequences).data, &[product]);
            assert_eq!(m.heap.view(product).data, &[empty, raw]);
            let subset = m.heap.symbolic(Kind::Subset, 0, &[sequences]);
            assert_eq!(m.symbolic_finiteness(subset), Ok(true));
            let outer_sequences = m.heap.symbolic(Kind::SeqSet, 0, &[sequences]);
            assert_eq!(m.symbolic_finiteness(outer_sequences), Ok(false));
            let finite_union = m.heap.symbolic(Kind::Union, 0, &[sequences, empty]);
            assert_eq!(m.symbolic_finiteness(finite_union), Ok(true));
            let unknown = m.heap.symbolic(Kind::Union, 0, &[sequences, zero]);
            let error = m.symbolic_finiteness(unknown).unwrap_err();
            assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
            assert!(error.message().contains("unproved factor"));
            let nonempty = m.heap.interval(0, 0);
            let malformed = m.heap.symbolic(Kind::Product, 0, &[nonempty, zero]);
            let malformed_sequences = m.heap.symbolic(Kind::SeqSet, 0, &[malformed]);
            assert!(m.symbolic_finiteness(malformed_sequences).is_err());

            assert_eq!(m.work_cardinality(raw), Ok(2));
            m.set_fuel(0);
            assert!(m.symbolic_finiteness(sequences).is_err());
            assert!(!m.heap.poisoned());
        }
    }

    #[test]
    fn finite_product_cardinality_accepts_canonical_mixed_axes() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let one = Handle::int(1).unwrap();
            let raw = m.heap.set_from(&mut vec![one, zero, one]);
            let axis = m.heap.interval(0, 0);
            let product = m.heap.symbolic(Kind::Product, 0, &[raw, axis]);
            m.finite_product(product).unwrap();

            assert_eq!(m.product_cardinality(product).unwrap().as_int(), Some(2));

            assert_eq!(m.heap.kind(product), Some(Kind::Product));
            assert!(m.member(zero, product).is_err());
            let mixed = m.heap.set_from(&mut vec![zero, Handle::TRUE]);
            let empty = m.heap.empty_set();
            let functions = m.heap.symbolic(Kind::FcnSet, 0, &[empty, mixed]);
            let skipped = m.heap.symbolic(Kind::Product, 0, &[functions, axis]);
            assert_eq!(m.product_cardinality(skipped).unwrap().as_int(), Some(1));

            let demanded = m.heap.symbolic(Kind::Product, 0, &[mixed, axis]);
            assert_eq!(m.product_cardinality(demanded).unwrap().as_int(), Some(2));
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
    fn source_order_factors_keep_the_empty_first_answer_in_both_profiles() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let empty = m.heap.empty_set();
            let empty_first = m.heap.symbolic(Kind::Product, 0, &[empty, zero]);
            let functions = m.heap.symbolic(Kind::FcnSet, 0, &[empty, zero]);
            assert_eq!(m.symbolic_finiteness(empty_first), Ok(true));
            assert_eq!(m.symbolic_finiteness(functions), Ok(true));
            let scalar_first = m.heap.symbolic(Kind::Product, 0, &[zero, empty]);
            assert!(m.symbolic_finiteness(scalar_first).is_err());
            assert!(!m.heap.poisoned());
        }
    }

    #[test]
    fn boolean_finiteness_keeps_unknown_factor_refusals() {
        for semantics in [Semantics::default(), Semantics::tlc()] {
            let mut m = Machine::new(
                Arc::new(Store::with_semantics(true, semantics)),
                0,
                Opts::default(),
                Memo::new(&[]),
            );
            let zero = Handle::int(0).unwrap();
            let raw = m.heap.set_from(&mut vec![zero, Handle::TRUE]);
            let empty = m.heap.empty_set();
            for kind in [Kind::Nat, Kind::Int] {
                let infinite = m.heap.symbolic(kind, 0, &[]);
                for axes in [[infinite, raw], [raw, infinite]] {
                    let product = m.heap.symbolic(Kind::Product, 0, &axes);
                    assert!(!m.symbolic_finiteness(product).unwrap());
                    assert_eq!(m.heap.kind(product), Some(Kind::Product));

                    assert!(m.product_cardinality(product).is_err());
                }
                let nested = m.heap.symbolic(Kind::Product, 0, &[infinite, empty]);
                let outer = m.heap.symbolic(Kind::Product, 0, &[nested, raw]);
                assert!(m.symbolic_finiteness(outer).unwrap());
                for axes in [[infinite, zero], [zero, infinite]] {
                    let invalid = m.heap.symbolic(Kind::Product, 0, &axes);
                    assert!(m.symbolic_finiteness(invalid).is_err());
                }
                let invalid = m.heap.symbolic(kind, 0, &[zero]);
                let product = m.heap.symbolic(Kind::Product, 0, &[raw, invalid]);
                assert!(m.symbolic_finiteness(product).is_err());
            }
            assert!(!m.heap.poisoned());
        }
    }
}
