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
        if set.order() != self.ctx.order
            || base.order() != self.ctx.order
            || base.stored_elements().is_none_or(|v| v.len() < 4)
        {
            return Ok(None);
        }
        let Some((outer, inner, test)) = self.leading_subset_guard(quantifier) else {
            return Ok(None);
        };
        let Expr::Builtin {
            op: BuiltinOp::Eq,
            args,
        } = &self.ir.node(test).expr
        else {
            return Ok(None);
        };
        // A settled slot can itself hold a deferred set provider. Requiring
        // explicit value trees prevents speculative comparison or sorting
        // from invoking that provider before the empty witness is tried.
        for &operand in args {
            let mut root = operand;
            while let Expr::RecordSelect { record, .. } = &self.ir.node(root).expr {
                root = *record;
            }
            let value = match &self.ir.node(root).expr {
                Expr::Local(r) if r.up == 0 && r.slot == inner => continue,
                Expr::Local(r) if r.up == 0 && r.slot == outer => return Ok(None),
                Expr::Var { var, primed: false } => {
                    let name = &self.ir.decl(self.ir.vars()[var.index()]).name;
                    self.state_variable(&st.s0, name, Some(var.index()))
                        .ok()
                        .flatten()
                        .cloned()
                }
                Expr::Const(c) => match self.consts.get(c.index()) {
                    Some(Some(crate::ConstBinding::Value(v))) => Some(v.clone()),
                    _ => None,
                },
                _ => self.settled(env, root),
            };
            if value.is_none_or(|v| v.require_materialized(self.ctx).is_err()) {
                return Ok(None);
            }
        }
        if Value::Set(base.clone())
            .require_materialized(self.ctx)
            .is_err()
        {
            return Ok(None);
        }
        let mut eligible = Vec::new();
        let mut inner_env = env.clone();
        let Ok(elements) = self.explore(base) else {
            return Ok(None);
        };
        for value in elements.iter() {
            self.loop_work()?;
            inner_env.bind(inner.index(), Sv::Val(value.clone()));
            match self.eval_v(&mut inner_env, st, test) {
                Ok(Value::Bool(true)) => eligible.push(value.clone()),
                Ok(Value::Bool(false)) => {}
                // Let the original evaluation raise any demanded failure.
                _ => return Ok(None),
            }
        }
        if eligible.len() == elements.len() {
            return Ok(None);
        }
        let Ok(filtered) = self.filtered_set(eligible) else {
            return Ok(None);
        };
        let Value::Set(restricted) = Value::subset(filtered, self.ctx) else {
            return Ok(None);
        };
        Ok(Some(restricted))
    }
    /// Returns the two distinct lexical bindings and the predicate of a
    /// leading universal guard over the outer subset itself.
    fn leading_subset_guard(
        &self,
        quantifier: ExprId,
    ) -> Option<(dive_ir::Slot, dive_ir::Slot, ExprId)> {
        let Expr::Quant {
            kind: QuantKind::Exists,
            bounds,
            body,
        } = &self.ir.node(quantifier).expr
        else {
            return None;
        };
        let outer = single_slot(bounds)?;
        let Expr::Junction {
            kind: JunctionKind::And,
            items,
            ..
        } = &self.ir.node(*body).expr
        else {
            return None;
        };
        let first = items.first()?;
        let Expr::Quant {
            kind: QuantKind::Forall,
            bounds: inner_bounds,
            body: test,
        } = &self.ir.node(*first).expr
        else {
            return None;
        };
        let inner = single_slot(inner_bounds)?;
        let domain = inner_bounds[0].domain?;
        if !matches!(&self.ir.node(domain).expr, Expr::Local(r) if r.up == 0 && r.slot == outer) {
            return None;
        }
        Some((outer, inner, *test))
    }
}

fn single_slot(bounds: &[Bound]) -> Option<dive_ir::Slot> {
    let [bound] = bounds else { return None };
    let [slot] = *bound.vars else { return None };
    (!bound.tuple).then_some(slot)
}
