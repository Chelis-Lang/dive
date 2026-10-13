//! One complete, ordered, single-candidate Init operation, without enumeration.

use std::sync::Arc;

use dive_eval::{Scope, SlotView, St};
use dive_ir::{BuiltinOp, Expr, ExprId, JunctionKind, OpRef, VarId};
use dive_model::Model;
use dive_value::Value;

use crate::State;

const UNSUPPORTED: &str = "private register Init: the independent single-candidate operation cannot determine every assignment from empty slots";

/// One complete single-variable equality, with no candidate-dependent filters.
/// `None` delegates to the deterministic operation; it grants no random access.
pub(crate) fn random_check(model: &Model, witness: &State) -> Result<Option<bool>, String> {
    if model.ir.vars().len() != 1 || model.init.len() != 1 {
        return Ok(None);
    }
    let operation = Initial {
        model,
        slots: vec![None],
    };
    let init = &model.init[0].pred;
    operation.random_predicate(&init.scope, init.expr, witness, 0)
}

pub(crate) fn check(model: &Model, witness: &State) -> Result<bool, String> {
    let mut operation = Initial {
        model,
        slots: vec![None; model.ir.vars().len()],
    };
    for init in &model.init {
        if !operation.predicate(&init.pred.scope, init.pred.expr, 0)? {
            return Ok(false);
        }
    }
    if operation.slots.iter().any(Option::is_none) {
        return Err(UNSUPPORTED.to_owned());
    }
    for (actual, expected) in operation.slots.iter().zip(witness.iter()) {
        let (Some(actual), Some(expected)) = (actual, expected) else {
            return Err(UNSUPPORTED.to_owned());
        };
        if !model
            .evaluator
            .equal_values(actual, expected)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

struct Initial<'a> {
    model: &'a Model,
    slots: Vec<Option<Value>>,
}

impl Initial<'_> {
    fn random_predicate(
        &self,
        scope: &Scope,
        expr: ExprId,
        witness: &State,
        depth: u32,
    ) -> Result<Option<bool>, String> {
        if depth > 128 {
            return Ok(None);
        }
        let ev = &self.model.evaluator;
        match self.model.ir.expr(expr) {
            Expr::Apply {
                callee: OpRef::Def(_),
                args,
            } if !args.is_empty() => Ok(None),
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                let (inner, def) = ev.scope_call(scope, expr).map_err(|e| e.to_string())?;
                if ev.is_value_override(def)
                    || ev.precomputed(def).is_some()
                    || self.model.ir.def(def).std.is_some()
                {
                    Ok(None)
                } else {
                    self.random_predicate(&inner, self.model.ir.def(def).body, witness, depth + 1)
                }
            }
            Expr::Selected(body) | Expr::Label { body, .. } => {
                self.random_predicate(scope, *body, witness, depth + 1)
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                let Some(var) = self.variable(scope, args[0], 0)? else {
                    return Ok(None);
                };
                let Some(Some(candidate)) = witness.get(var.index()) else {
                    return Ok(None);
                };
                let st = St::initial(Arc::from(self.slots.clone()));
                ev.random_element_member(scope, args[1], &st, candidate, 65_536)
                    .map_err(|e| format!("direct RandomElement Init relation: {e}"))
            }
            _ => Ok(None),
        }
    }

    fn variable(&self, scope: &Scope, expr: ExprId, depth: u32) -> Result<Option<VarId>, String> {
        if depth > 128 {
            return Err(UNSUPPORTED.to_owned());
        }
        Ok(match self.model.ir.expr(expr) {
            Expr::Var { var, primed: false } => Some(*var),
            Expr::SubstRef {
                decl,
                primed: false,
            } => match self.model.evaluator.scope_subst_slot(scope, *decl) {
                SlotView::Unset => self.model.ir.decl(*decl).var,
                SlotView::Lazy(expr, scope) => return self.variable(&scope, expr, depth + 1),
                _ => None,
            },
            _ => None,
        })
    }

    fn predicate(&mut self, scope: &Scope, expr: ExprId, depth: u32) -> Result<bool, String> {
        if depth > 128 {
            return Err(UNSUPPORTED.to_owned());
        }
        let ev = &self.model.evaluator;
        match self.model.ir.expr(expr) {
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &item in items {
                    if !self.predicate(scope, item, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                let (inner, def) = ev.scope_call(scope, expr).map_err(|e| e.to_string())?;
                if ev.is_value_override(def)
                    || ev.precomputed(def).is_some()
                    || self.model.ir.def(def).std.is_some()
                {
                    self.filter(scope, expr)
                } else {
                    self.predicate(&inner, self.model.ir.def(def).body, depth + 1)
                }
            }
            Expr::Subst { body, .. } => {
                let inner = ev.scope_subst(scope, expr).map_err(|e| e.to_string())?;
                self.predicate(&inner, *body, depth + 1)
            }
            Expr::Selected(body) | Expr::Label { body, .. } => {
                self.predicate(scope, *body, depth + 1)
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                if let Some(var) = self.variable(scope, args[0], 0)? {
                    let st = St::initial(Arc::from(self.slots.clone()));
                    let value = ev
                        .eval_scoped(scope, args[1], &st)
                        .map_err(|e| format!("{UNSUPPORTED}: {e}"))?;
                    if let Some(previous) = &self.slots[var.index()] {
                        ev.equal_values(previous, &value).map_err(|e| e.to_string())
                    } else {
                        self.slots[var.index()] = Some(value);
                        Ok(true)
                    }
                } else {
                    self.filter(scope, expr)
                }
            }
            // These relation forms can visit other candidates or bind slots.
            // Do not evaluate one selected by the supplied trace.
            Expr::Junction {
                kind: JunctionKind::Or,
                ..
            }
            | Expr::Quant { .. }
            | Expr::If { .. }
            | Expr::Case { .. }
            | Expr::Let { .. }
            | Expr::Builtin {
                op: BuiltinOp::In, ..
            } => Err(UNSUPPORTED.to_owned()),
            _ => self.filter(scope, expr),
        }
    }

    fn filter(&self, scope: &Scope, expr: ExprId) -> Result<bool, String> {
        let st = St::initial(Arc::from(self.slots.clone()));
        match self.model.evaluator.eval_scoped(scope, expr, &st) {
            Ok(Value::Bool(value)) => Ok(value),
            Ok(value) => Err(format!(
                "{UNSUPPORTED}: a filter returned {}",
                value.kind_name()
            )),
            Err(error) => Err(format!("{UNSUPPORTED}: {error}")),
        }
    }
}
