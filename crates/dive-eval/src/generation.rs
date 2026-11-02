//! Indexed contexts for constructive Next generation only.

use std::sync::Arc;

use dive_ir::{Bound, ExprId};
use dive_value::{SetKind, Value};

use crate::core::{Core, is_enumerable};
use crate::env::{Env, St, Sv};
use crate::error::{Ec, EvalError, Result};

/// A bounded cyclic index permutation. A noncoprime sampled stride becomes one.
pub(crate) struct IndexOrder {
    len: u64,
    start: u64,
    index: u64,
    stride: u64,
    remaining: u64,
}

impl IndexOrder {
    pub(crate) fn new(len: u64, choose: &mut dyn FnMut(u64) -> u64) -> Result<Self> {
        let start = if len == 0 { 0 } else { draw(len, choose)? };
        let stride = if len <= 1 {
            1
        } else {
            let proposed = draw(len - 1, choose)? + 1;
            if gcd(proposed, len) == 1 { proposed } else { 1 }
        };
        Ok(Self {
            len,
            start,
            index: start,
            stride,
            remaining: len,
        })
    }

    pub(crate) fn current(&self) -> Option<u64> {
        (self.remaining != 0).then_some(self.index)
    }

    #[allow(clippy::cast_possible_truncation, reason = "modulus is a positive u64")]
    pub(crate) fn advance(&mut self) -> bool {
        self.remaining = self.remaining.saturating_sub(1);
        if self.remaining == 0 {
            self.index = self.start;
            self.remaining = self.len;
            return false;
        }
        // Both summands fit u64; the widened sum cannot overflow.
        self.index =
            ((u128::from(self.index) + u128::from(self.stride)) % u128::from(self.len)) as u64;
        true
    }
}

fn draw(len: u64, choose: &mut dyn FnMut(u64) -> u64) -> Result<u64> {
    let index = choose(len);
    if index >= len {
        return Err(EvalError::unsupported(
            "constructive generation index outside its domain",
        ));
    }
    Ok(index)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

enum Domain {
    Stored(Arc<[Value]>),
    Interval { low: i64, len: u64 },
}

impl Domain {
    fn len(&self) -> Result<u64> {
        match self {
            Self::Stored(values) => u64::try_from(values.len())
                .map_err(|_| EvalError::resource_limit("constructive generation domain size")),
            Self::Interval { len, .. } => Ok(*len),
        }
    }

    fn at(&self, index: u64) -> Result<Value> {
        match self {
            Self::Stored(values) => usize::try_from(index)
                .ok()
                .and_then(|index| values.get(index))
                .cloned()
                .ok_or_else(|| EvalError::bug("constructive stored-domain index")),
            Self::Interval { low, len } if index < *len => {
                let value = i64::try_from(i128::from(*low) + i128::from(index))
                    .map_err(|_| EvalError::resource_limit("constructive interval value"))?;
                Ok(Value::Int(value))
            }
            Self::Interval { .. } => Err(EvalError::bug("constructive interval index")),
        }
    }
}

struct Column {
    domain: Arc<Domain>,
    slots: Vec<usize>,
    tuple: bool,
    order: IndexOrder,
}

/// Original binder admission, followed by bounded randomized context traversal.
pub(crate) struct Contexts {
    columns: Vec<Column>,
    done: bool,
}

impl Contexts {
    pub(crate) fn bind(&self, core: &Arc<Core>, env: &mut Env) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        core.loop_work()?;
        for column in &self.columns {
            let Some(index) = column.order.current() else {
                return Ok(false);
            };
            let value = column.domain.at(index)?;
            if column.tuple {
                let Value::Fcn(function) = &value else {
                    return Err(tuple_error());
                };
                let Some(tuple) = function.as_tuple() else {
                    return Err(tuple_error());
                };
                if tuple.len() != column.slots.len() {
                    return Err(tuple_error());
                }
                for (&slot, value) in column.slots.iter().zip(tuple) {
                    env.bind(slot, Sv::Val(value.clone()));
                }
            } else if let Some(&slot) = column.slots.first() {
                env.bind(slot, Sv::Val(value));
            } else {
                return Err(EvalError::bug("empty constructive binding unit"));
            }
        }
        Ok(true)
    }

    pub(crate) fn advance(&mut self) {
        for column in &mut self.columns {
            if column.order.advance() {
                return;
            }
        }
        self.done = true;
    }
}

fn tuple_error() -> EvalError {
    EvalError::coded(
        Ec::ArgumentMismatch,
        "Argument mismatch in operator application.%1",
    )
}

impl Core {
    pub(crate) fn generation_contexts(
        self: &Arc<Self>,
        env: &mut Env,
        state: &St,
        bounds: &[Bound],
        _expression: ExprId,
        choose: &mut dyn FnMut(u64) -> u64,
    ) -> Result<Contexts> {
        // Construct every domain before examining emptiness or any context.
        // Choice setup is separate so an invalid chooser cannot skip a later
        // source-domain error.
        let mut admitted = Vec::new();
        for bound in bounds {
            let Some(expr) = bound.domain else {
                return Err(EvalError::bug("unbounded constructive binder"));
            };
            let value = self.eval_v(env, state, expr)?;
            let Value::Set(set) = &value else {
                return Err(self.non_enumerable_bound(&value, expr));
            };
            if !is_enumerable(&value) {
                return Err(self.non_enumerable_bound(&value, expr));
            }
            let domain = if let Some((low, high)) = set.as_interval() {
                let (Some(low), Some(high)) = (low.to_i64(), high.to_i64()) else {
                    return Err(EvalError::unsupported(
                        "constructive generation interval endpoints outside i64",
                    ));
                };
                // Preserve the profile's own interval cardinality error.
                set.size(self.ctx)?;
                let len = u64::try_from((i128::from(high) - i128::from(low) + 1).max(0))
                    .map_err(|_| EvalError::resource_limit("constructive interval cardinality"))?;
                Domain::Interval { low, len }
            } else if matches!(set.kind(), SetKind::Enum | SetKind::Raw) {
                // Existing class-specific demand, without a second element vector.
                Domain::Stored(self.explore(set)?)
            } else {
                return Err(EvalError::unsupported(
                    "constructive generation requires an interval or stored finite binder domain",
                ));
            };
            admitted.push((bound, Arc::new(domain)));
        }
        let mut columns = Vec::new();
        for (bound, domain) in admitted {
            if bound.tuple {
                columns.push(Column {
                    order: IndexOrder::new(domain.len()?, choose)?,
                    domain,
                    slots: bound.vars.iter().map(|slot| slot.index()).collect(),
                    tuple: true,
                });
            } else {
                for slot in &bound.vars {
                    columns.push(Column {
                        order: IndexOrder::new(domain.len()?, choose)?,
                        domain: domain.clone(),
                        slots: vec![slot.index()],
                        tuple: false,
                    });
                }
            }
        }
        let done = columns
            .iter()
            .any(|column| column.order.current().is_none());
        Ok(Contexts { columns, done })
    }
}

#[cfg(test)]
mod tests {
    use super::IndexOrder;

    #[test]
    fn every_length_and_draw_visits_each_index_once() -> crate::error::Result<()> {
        for len in 1..24 {
            for draw in 0..len {
                let mut order = IndexOrder::new(len, &mut |bound| draw % bound)?;
                let mut seen = std::collections::BTreeSet::new();
                loop {
                    assert!(seen.insert(order.current()));
                    if !order.advance() {
                        break;
                    }
                }
                assert_eq!(seen.len() as u64, len);
            }
        }
        Ok(())
    }

    #[test]
    fn empty_never_calls_chooser_and_bad_chooser_is_typed() -> crate::error::Result<()> {
        let mut calls = 0;
        let order = IndexOrder::new(0, &mut |_| {
            calls += 1;
            0
        })?;
        assert_eq!(order.current(), None);
        assert_eq!(calls, 0);
        assert!(IndexOrder::new(4, &mut |bound| bound).is_err());
        Ok(())
    }
}
