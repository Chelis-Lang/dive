//! Inverted positions for an exact, already materialized state-set membership.
//! The only cached information is word data. Resolve the actual binding and
//! the first necessary source guard again for every use.
use std::collections::HashMap;
use std::sync::Arc;

use dive_eval::{Scope, SlotView};
use dive_ir::{Arg, BuiltinOp, DefScope, Effects, Expr, ExprId, JunctionKind, OpRef, Pass, VarId};
use dive_value::{SetKind, Value};

use super::{TemporalError, Truth, Word};

#[derive(Default)]
pub(super) struct Membership {
    // Attempt one root variable per word, including a remembered failed build.
    variable: Option<VarId>,
    index: Option<Index>,
}

struct Index {
    sample: Option<Value>,
    positions: HashMap<Value, Arc<Vec<usize>>>,
}

fn scalar_pair(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (Value::Bool(_), Value::Bool(_))
            | (Value::Int(_), Value::Int(_))
            | (Value::Str(_), Value::Str(_))
    )
}

// On these shapes evaluator equality is total and agrees with Value's exact
// hash/equality. A heterogeneous set, lazy value or other function takes the
// ordinary evaluator, including its comparison errors.
fn compatible(a: &Value, b: &Value) -> bool {
    if scalar_pair(a, b) {
        return true;
    }
    let (Some(a), Some(b)) = (
        a.as_fcn().and_then(|f| f.as_record()),
        b.as_fcn().and_then(|f| f.as_record()),
    ) else {
        return false;
    };
    a.len() == b.len()
        && a.len() <= 16
        && a.iter()
            .zip(b)
            .all(|((ka, va), (kb, vb))| ka == kb && scalar_pair(va, vb))
}

impl Word<'_> {
    pub(super) fn sparse_fairness(
        &mut self,
        action: ExprId,
        sub: ExprId,
        scope: &Scope,
        strong: bool,
    ) -> Result<Option<Truth>, TemporalError> {
        if let Some(key) = self.subscript_key(sub, scope)
            && self
                .stutters
                .as_ref()
                .is_some_and(|(stored, _)| *stored == key)
            && !self
                .model
                .ir
                .node(action)
                .effects
                .intersects(Effects::IMPURE)
            && let Some((var, binding)) = self.first_membership(action, scope, true)
            && let Some(positions) = self.members(var, &binding)?
        {
            self.check_atom_context(action, scope)?;
            self.check_scoped_context_mode(sub, scope, 0, &mut 0, false)?;
            self.charge(positions.len() + self.states.len().div_ceil(64))?;
            let mut recurrent = false;
            for (j, &i) in positions.iter().enumerate() {
                if j % 256 == 0 {
                    self.model.evaluator.poll_host_control()?;
                }
                if self.stutters.as_ref().is_some_and(|(_, col)| col.at(i)) {
                    continue;
                }
                let next = if i + 1 == self.states.len() {
                    self.back_to
                } else {
                    i + 1
                };
                recurrent |= self.action_at(i, next, action, scope)? && i >= self.back_to;
            }
            if recurrent {
                return Ok(Some(Truth::constant(self.states.len(), true)));
            }
            let cycle = &positions[positions.partition_point(|i| *i < self.back_to)..];
            return self
                .fair_enabled(action, sub, scope, strong, cycle.iter().copied())
                .map(Some);
        }
        Ok(None)
    }

    // Follow only lazy, non-evaluating source wrappers. In particular do not
    // inspect a later conjunct, substitute a caller's local by its spelling,
    // or speculate by evaluating an arbitrary actual.
    pub(super) fn first_membership(
        &self,
        mut expr: ExprId,
        scope: &Scope,
        action: bool,
    ) -> Option<(VarId, Value)> {
        let ev = &self.model.evaluator;
        if !scope.shares_pure_source_walk(scope, ev, expr) {
            return None;
        }
        let mut sc = scope.clone();
        for _ in 0..24 {
            match self.model.ir.expr(expr) {
                Expr::Apply {
                    callee: OpRef::Def(d),
                    args,
                } if action => {
                    let def = self.model.ir.def(*d);
                    if ev.redirected(*d) != *d
                        || ev.is_value_override(*d)
                        || def.recursive
                        || def.std.is_some()
                        || def.scope != DefScope::TopLevel
                        || !args.iter().all(|a| match a {
                            Arg::Expr { pass: Pass::Lazy, .. } => true,
                            Arg::Expr { expr, .. } => matches!(self.model.ir.expr(*expr),
                                Expr::Local(r) if matches!(ev.scope_slot(&sc, *r), SlotView::Value(_))),
                            Arg::Op { .. } => false,
                        })
                    {
                        return None;
                    }
                    sc = ev.scope_apply(&sc, *d, args).ok()?;
                    expr = def.body;
                }
                Expr::Let { body, defs } if action => {
                    // Ordinary evaluation forces strict LET values even when the
                    // leading guard is false. Only walk non-evaluating wrappers.
                    if defs.iter().any(|d| {
                        matches!(
                            self.model.ir.def(*d).scope,
                            DefScope::LetValue {
                                pass: Pass::Strict,
                                ..
                            }
                        )
                    }) {
                        return None;
                    }
                    sc = ev.scope_let(&sc, expr).ok()?;
                    expr = *body;
                }
                Expr::Junction {
                    kind: JunctionKind::And,
                    items,
                    ..
                } if action => {
                    expr = *items.first()?;
                }
                Expr::Builtin {
                    op: BuiltinOp::In,
                    args,
                } => {
                    let [mut key, set] = *args.as_ref() else {
                        return None;
                    };
                    let Expr::Var { var, primed: false } = self.model.ir.expr(set) else {
                        return None;
                    };
                    for _ in 0..24 {
                        let Expr::Local(local) = self.model.ir.expr(key) else {
                            return None;
                        };
                        match ev.scope_slot(&sc, *local) {
                            SlotView::Value(v) if compatible(&v, &v) => return Some((*var, v)),
                            SlotView::Lazy(actual, caller) => {
                                key = actual;
                                sc = caller;
                            }
                            _ => return None,
                        }
                    }
                    return None;
                }
                _ => return None,
            }
        }
        None
    }

    pub(super) fn members(
        &mut self,
        var: VarId,
        key: &Value,
    ) -> Result<Option<Arc<Vec<usize>>>, TemporalError> {
        if self.membership.variable.is_none() {
            self.membership.variable = Some(var);
            self.membership.index = self.build_membership(var)?;
        }
        let Some(index) = self
            .membership
            .index
            .as_ref()
            .filter(|_| self.membership.variable == Some(var))
        else {
            return Ok(None);
        };
        if index.sample.as_ref().is_some_and(|v| !compatible(v, key)) {
            return Ok(None);
        }
        Ok(Some(index.positions.get(key).cloned().unwrap_or_default()))
    }

    fn build_membership(&mut self, var: VarId) -> Result<Option<Index>, TemporalError> {
        // Bound the number of retained word references and key clones. Every key
        // is a scalar or a shallow record backed by existing immutable word data.
        const MAX_ENTRIES: usize = 65_536;
        const MAX_KEYS: usize = 4096;
        if self.states.len() > MAX_ENTRIES {
            return Ok(None);
        }
        #[allow(
            clippy::mutable_key_type,
            reason = "admission allows only immutable scalars and flat records of scalars; Value hash/equality does not depend on its internal caches"
        )]
        let mut positions: HashMap<Value, Arc<Vec<usize>>> = HashMap::new();
        let mut sample: Option<Value> = None;
        let mut entries: usize = 0;
        for (i, state) in self.states.iter().enumerate() {
            if i % 128 == 0 {
                self.model.evaluator.poll_host_control()?;
            }
            let Some(set) = state[var.index()].as_ref().and_then(Value::as_set) else {
                return Ok(None);
            };
            if set.kind() != SetKind::Enum {
                return Ok(None);
            }
            let Some(items) = set.stored_elements() else {
                return Ok(None);
            };
            entries = entries.saturating_add(items.len());
            if entries > MAX_ENTRIES {
                return Ok(None);
            }
            for (j, key) in items.iter().enumerate() {
                if j > 0 && j % 128 == 0 {
                    self.model.evaluator.poll_host_control()?;
                }
                if !compatible(sample.as_ref().unwrap_or(key), key) {
                    return Ok(None);
                }
                sample.get_or_insert_with(|| key.clone());
                let rows = positions.entry(key.clone()).or_default();
                Arc::make_mut(rows).push(i);
                if positions.len() > MAX_KEYS {
                    return Ok(None);
                }
            }
        }
        self.charge(self.states.len() + entries)?;
        Ok(Some(Index { sample, positions }))
    }
}
