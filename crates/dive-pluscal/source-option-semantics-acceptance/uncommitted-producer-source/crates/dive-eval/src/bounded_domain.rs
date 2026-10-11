//! Structural preflight for finite binder domains. No deferred predicate is
//! evaluated here, and every component is bounded before enumeration allocates.

use dive_value::{Ctx, Set, SetKind, Value};

use crate::{EvalError, Result};

pub(crate) struct Budget {
    remaining: usize,
    limit: usize,
    depth: u32,
    ctx: Ctx,
}

impl Budget {
    pub(crate) const fn new(limit: usize, depth: u32, ctx: Ctx) -> Self {
        Self {
            remaining: limit,
            limit,
            depth,
            ctx,
        }
    }

    fn charge(&mut self, count: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or_else(|| EvalError::resource_limit("quantifier domain materialization work"))?;
        Ok(())
    }

    fn component(&mut self, value: &Value, depth: u32) -> Result<usize> {
        let Value::Set(set) = value else {
            return Err(EvalError::unsupported(
                "finite binder domain component is not a set",
            ));
        };
        self.set(set, depth)
    }

    fn value(&mut self, value: &Value, depth: u32) -> Result<()> {
        if depth >= self.depth {
            return Err(EvalError::resource_limit("quantifier domain nesting"));
        }
        match value {
            // Atomic infinite-set values require no enumeration; they are not
            // themselves accepted as binder domains.
            Value::Set(set)
                if matches!(set.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings) =>
            {
                Ok(())
            }
            Value::Set(set) => self.set(set, depth).map(|_| ()),
            Value::Fcn(function) => {
                self.charge(function.len().checked_mul(2).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                for (key, value) in function.iter() {
                    self.value(&key, depth + 1)?;
                    self.value(value, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn explicit(&mut self, set: &Set, depth: u32) -> Result<usize> {
        // Use a cardinality upper bound without normalizing raw operands:
        // TLC's normalizing size/enumeration runs only after preflight,
        // in its original domain/range error order.
        if let Some(raw) = set.raw_elements() {
            self.charge(raw.len())?;
            for value in raw {
                self.value(value, depth)?;
            }
            Ok(raw.len())
        } else {
            let size = usize::try_from(set.size(self.ctx)?)
                .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?;
            self.charge(size)?;
            for value in set.elements(self.ctx)?.iter() {
                self.value(value, depth)?;
            }
            Ok(size)
        }
    }

    pub(crate) fn set(&mut self, set: &Set, depth: u32) -> Result<usize> {
        if depth >= self.depth {
            return Err(EvalError::resource_limit("quantifier domain nesting"));
        }
        let child_depth = depth + 1;
        let width;
        let count = match set.kind() {
            SetKind::Enum | SetKind::Raw => self.explicit(set, child_depth)?,
            SetKind::Interval => {
                let (low, high) = set
                    .as_interval()
                    .ok_or_else(|| EvalError::bug("interval kind without bounds"))?;
                let size = if high < low {
                    0
                } else {
                    usize::try_from(high.to_big() - low.to_big() + 1_u32)
                        .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?
                };
                self.charge(size)?;
                size
            }
            SetKind::Subset => {
                let components = set.components();
                let base = &components[0].1;
                width = self.component(base, child_depth)?;
                let count = u32::try_from(width)
                    .ok()
                    .and_then(|n| 1_usize.checked_shl(n))
                    .filter(|n| *n <= self.limit)
                    .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                self.charge(count.checked_mul(width.max(1)).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                count
            }
            SetKind::FcnSet => {
                let (domain, range) = set
                    .as_fcn_set()
                    .ok_or_else(|| EvalError::bug("function-set kind without components"))?;
                width = self.component(domain, child_depth)?;
                if width == 0 {
                    // TLC never touches the range of the singleton empty function.
                    self.charge(1)?;
                    return Ok(1);
                }
                let range_count = self.component(range, child_depth)?;
                let count = u32::try_from(width)
                    .ok()
                    .and_then(|n| range_count.checked_pow(n))
                    .filter(|n| *n <= self.limit)
                    .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                self.charge(
                    count
                        .checked_mul(width)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(|| {
                            EvalError::resource_limit("quantifier domain materialization work")
                        })?,
                )?;
                count
            }
            SetKind::Product | SetKind::RcdSet => {
                width = set.as_product().map_or_else(
                    || {
                        set.as_rcd_set()
                            .map_or(0, <[(dive_value::Atom, Value)]>::len)
                    },
                    <[Value]>::len,
                );
                self.charge(width)?;
                let mut count = 1_usize;
                for (_, component) in set.components() {
                    count = count
                        .checked_mul(self.component(&component, child_depth)?)
                        .filter(|n| *n <= self.limit)
                        .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
                }
                self.charge(count.checked_mul(width.max(1)).ok_or_else(|| {
                    EvalError::resource_limit("quantifier domain materialization work")
                })?)?;
                count
            }
            _ => {
                return Err(EvalError::unsupported(
                    "bounded bindings require structurally finite domains; infinite and deferred sets are not supported",
                ));
            }
        };
        if count > self.limit {
            return Err(EvalError::resource_limit("quantifier domain entries"));
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "test assertions")]
    use super::Budget;
    use dive_value::{Ctx, Value};
    #[test]
    fn structural_preflight_does_not_normalize_raw_operands() {
        let context = Ctx::tlc();
        let value = Value::set_raw(vec![2.into(), 1.into(), 1.into()], context);
        let set = value.as_set().expect("constructed set");
        assert!(set.raw_elements().is_some());
        assert_eq!(Budget::new(3, 256, context).set(set, 0).ok(), Some(3));
        assert!(set.raw_elements().is_some());
    }
}
