//! Independent reduction of a wide existential domain in concrete action predicates.

use std::sync::Arc;

use dive_ir::{Bound, BuiltinOp, Expr, ExprId, JunctionKind, QuantKind};
use dive_value::{Set, SetKind, Value};

use crate::core::Core;
use crate::env::{Env, St, StateRef, Sv, control};
use crate::error::Result;

impl Core {
    /// On a concrete step, filter a powerset's *elements* when the first
    /// conjunct requires each selected element to pass a scalar equality.
    /// The caller still evaluates the entire original predicate per subset.
    pub(crate) fn guarded_subset(
        self: &Arc<Self>,
        env: &Env,
        st: &St,
        quantifier: ExprId,
        set: &Set,
    ) -> Result<Option<Set>> {
        if set.kind() != SetKind::Subset
            || (!st.is(control::ENABLED)
                && !matches!(st.s1, StateRef::Vals(_) | StateRef::Named(_)))
        {
            return Ok(None);
        }
        let parts = set.components();
        let [(None, Value::Set(base))] = parts.as_slice() else {
            return Ok(None);
        };
        if base.stored_elements().is_none_or(|v| v.len() < 4) {
            return Ok(None);
        }
        let Expr::Quant {
            kind: QuantKind::Exists,
            bounds,
            body,
        } = &self.ir.node(quantifier).expr
        else {
            return Ok(None);
        };
        let Some(outer) = single_slot(bounds) else {
            return Ok(None);
        };
        let Expr::Junction {
            kind: JunctionKind::And,
            items,
            ..
        } = &self.ir.node(*body).expr
        else {
            return Ok(None);
        };
        let Some(first) = items.first() else {
            return Ok(None);
        };
        let Expr::Quant {
            kind: QuantKind::Forall,
            bounds: inner_bounds,
            body: test,
        } = &self.ir.node(*first).expr
        else {
            return Ok(None);
        };
        let Some(inner) = single_slot(inner_bounds) else {
            return Ok(None);
        };
        let Some(domain) = inner_bounds[0].domain else {
            return Ok(None);
        };
        if !matches!(&self.ir.node(domain).expr, Expr::Local(r) if r.up == 0 && r.slot == outer) {
            return Ok(None);
        }
        let Expr::Builtin {
            op: BuiltinOp::Eq,
            args,
        } = &self.ir.node(*test).expr
        else {
            return Ok(None);
        };
        // No definition or delayed argument can read the candidate set,
        // perform an effect, or acquire a different value across candidates.
        for &operand in args {
            let mut arg = operand;
            while let Expr::RecordSelect { record, .. } = &self.ir.node(arg).expr {
                arg = *record;
            }
            match &self.ir.node(arg).expr {
                Expr::Local(r) if r.up == 0 && r.slot == inner => {}
                Expr::Local(r) if r.up == 0 && r.slot == outer => return Ok(None),
                Expr::Var { primed: false, .. } => {}
                Expr::Const(c)
                    if matches!(
                        self.consts.get(c.index()),
                        Some(Some(crate::ConstBinding::Value(_)))
                    ) => {}
                _ if self.settled(env, arg).is_some() => {}
                _ => return Ok(None),
            }
        }
        let mut eligible = Vec::new();
        let mut inner_env = env.clone();
        for value in self.explore(base)?.iter() {
            self.loop_work()?;
            inner_env.bind(inner.index(), Sv::Val(value.clone()));
            match self.eval_v(&mut inner_env, st, *test) {
                Ok(Value::Bool(true)) => eligible.push(value.clone()),
                Ok(Value::Bool(false)) => {}
                // Let the original evaluation raise any demanded failure.
                _ => return Ok(None),
            }
        }
        let Value::Set(restricted) = Value::subset(Value::set(eligible, self.ctx)?, self.ctx)
        else {
            return Ok(None);
        };
        Ok(Some(restricted))
    }
}

fn single_slot(bounds: &[Bound]) -> Option<dive_ir::Slot> {
    let [bound] = bounds else { return None };
    let [slot] = *bound.vars else { return None };
    (!bound.tuple).then_some(slot)
}
