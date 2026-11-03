//! Ordered single-candidate Init/Next construction, without checker enumeration.

use std::sync::Arc;

use dive_eval::{EvalError, Scope, SlotView, St};
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
    if witness
        .iter()
        .flatten()
        .any(|value| matches!(value, Value::Set(_) | Value::Fcn(_)))
    {
        return Ok(None);
    }
    let operation = Initial {
        model,
        slots: vec![None],
        source: None,
        choices: None,
    };
    let init = &model.init[0].pred;
    operation.random_predicate(&init.scope, init.expr, witness, 0)
}

pub(crate) fn check(model: &Model, witness: &State) -> Result<bool, EvalError> {
    let mut operation = Initial {
        model,
        slots: vec![None; model.ir.vars().len()],
        source: None,
        choices: None,
    };
    for init in &model.init {
        if !operation.predicate(&init.pred.scope, init.pred.expr, 0)? {
            return Ok(false);
        }
    }
    operation.matches(witness)
}

/// `None` is a structural decline before any source evaluation. Its caller may
/// use ordinary predicate validation only with reached random calls refused.
pub(crate) fn next(
    model: &Model,
    source: &State,
    witness: &State,
    choices: &crate::choices::Search,
) -> Result<Option<bool>, EvalError> {
    // Binding can freeze constant action arguments or retain uncached LET
    // scopes. Reconstruct those source-bound actions, never their producer
    // states or generator output. The unsplit raw predicate loses that context.
    if model
        .actions
        .iter()
        .any(|action| !action_shape(model, action.expr(), 0))
    {
        return Ok(None);
    }
    if model.actions.is_empty() {
        return Ok(Some(false));
    }
    let choice = if model.actions.len() == 1 {
        0
    } else {
        choices
            .branch(model.actions.len())
            .map_err(|error| EvalError::unsupported(error.message))?
    };
    let action = model
        .actions
        .get(choice)
        .ok_or_else(|| EvalError::unsupported("independent random Next action choice"))?;
    let mut operation = Initial {
        model,
        slots: vec![None; model.ir.vars().len()],
        source: Some(source),
        choices: Some(choices),
    };
    if !operation.predicate(&action.scope(), action.expr(), 0)? {
        return Ok(Some(false));
    }
    operation.matches(witness).map(Some)
}

fn action_shape(model: &Model, expr: ExprId, depth: u32) -> bool {
    if depth > 128 {
        return false;
    }
    match model.ir.expr(expr) {
        Expr::Def(def)
        | Expr::Apply {
            callee: OpRef::Def(def),
            ..
        } => {
            let def = model.evaluator.redirected(*def);
            model.evaluator.is_value_override(def)
                || model.evaluator.precomputed(def).is_some()
                || model.ir.def(def).std.is_some()
                || action_shape(model, model.ir.def(def).body, depth + 1)
        }
        Expr::Junction { items, .. } => items
            .iter()
            .all(|&expr| action_shape(model, expr, depth + 1)),
        Expr::Subst { body, .. }
        | Expr::Selected(body)
        | Expr::Label { body, .. }
        | Expr::Let { body, .. } => action_shape(model, *body, depth + 1),
        Expr::If { then, other, .. } => {
            action_shape(model, *then, depth + 1) && action_shape(model, *other, depth + 1)
        }
        Expr::Quant { .. }
        | Expr::Case { .. }
        | Expr::StepBox { .. }
        | Expr::StepAngle { .. }
        | Expr::Apply {
            callee: OpRef::Param(_),
            ..
        }
        | Expr::Local(_)
        | Expr::Builtin {
            op: BuiltinOp::In, ..
        } => false,
        _ => true,
    }
}

struct Initial<'a> {
    model: &'a Model,
    slots: Vec<Option<Value>>,
    source: Option<&'a State>,
    choices: Option<&'a crate::choices::Search>,
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
                let Some(var) = self
                    .variable(scope, args[0], 0)
                    .map_err(|e| e.to_string())?
                else {
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

    fn st(&self) -> St {
        let slots: State = Arc::from(self.slots.clone());
        if let Some(source) = self.source {
            St::action(source.clone(), slots)
        } else {
            St::initial(slots)
        }
    }

    fn matches(&self, witness: &State) -> Result<bool, EvalError> {
        for (actual, expected) in self.slots.iter().zip(witness.iter()) {
            let (Some(actual), Some(expected)) = (actual, expected) else {
                return Err(EvalError::unsupported(UNSUPPORTED));
            };
            if !self.model.evaluator.equal_values(actual, expected)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn variable(
        &self,
        scope: &Scope,
        expr: ExprId,
        depth: u32,
    ) -> Result<Option<VarId>, EvalError> {
        self.variable_side(scope, expr, self.source.is_some(), depth)
    }

    fn variable_side(
        &self,
        scope: &Scope,
        expr: ExprId,
        primed: bool,
        depth: u32,
    ) -> Result<Option<VarId>, EvalError> {
        if depth > 128 {
            return Err(EvalError::unsupported(UNSUPPORTED));
        }
        Ok(match self.model.ir.expr(expr) {
            Expr::Var {
                var,
                primed: actual,
            } if *actual == primed => Some(*var),
            Expr::Prime(body) if primed => {
                return self.variable_side(scope, *body, false, depth + 1);
            }
            Expr::SubstRef {
                decl,
                primed: actual,
            } if *actual == primed => match self.model.evaluator.scope_subst_slot(scope, *decl) {
                SlotView::Unset => self.model.ir.decl(*decl).var,
                SlotView::Lazy(expr, scope) => {
                    return self.variable_side(&scope, expr, primed, depth + 1);
                }
                _ => None,
            },
            _ => None,
        })
    }

    fn unchanged(&mut self, scope: &Scope, expr: ExprId, depth: u32) -> Result<bool, EvalError> {
        if depth > 128 {
            return Err(EvalError::unsupported(UNSUPPORTED));
        }
        let ev = &self.model.evaluator;
        if let Some(var) = self.variable_side(scope, expr, false, depth + 1)? {
            let value = self
                .source
                .and_then(|source| source.get(var.index()))
                .and_then(Option::as_ref)
                .ok_or_else(|| EvalError::unsupported(UNSUPPORTED))?
                .clone();
            if let Some(previous) = &self.slots[var.index()] {
                return ev.equal_values(previous, &value);
            }
            self.slots[var.index()] = Some(value);
            return Ok(true);
        }
        match self.model.ir.expr(expr) {
            Expr::Tuple(items) => {
                for &item in items {
                    if !self.unchanged(scope, item, depth + 1)? {
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
                let (inner, def) = ev.scope_call(scope, expr)?;
                self.unchanged(&inner, self.model.ir.def(def).body, depth + 1)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.unchanged(scope, *body, depth + 1)
            }
            _ => Err(EvalError::unsupported(
                "independent random Next UNCHANGED assignment",
            )),
        }
    }

    fn predicate(&mut self, scope: &Scope, expr: ExprId, depth: u32) -> Result<bool, EvalError> {
        if depth > 128 {
            return Err(EvalError::unsupported(UNSUPPORTED));
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
                let (inner, def) = ev.scope_call(scope, expr)?;
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
                let inner = ev.scope_subst(scope, expr)?;
                self.predicate(&inner, *body, depth + 1)
            }
            Expr::Selected(body) | Expr::Label { body, .. } => {
                self.predicate(scope, *body, depth + 1)
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => self.equality(scope, expr, args[0], args[1]),
            Expr::Let { body, .. } => {
                let inner = ev.scope_let(scope, expr)?;
                self.predicate(&inner, *body, depth + 1)
            }
            Expr::Junction {
                kind: JunctionKind::Or,
                items,
                ..
            } if self.source.is_some() => {
                if items.is_empty() {
                    return Ok(false);
                }
                let choice = self
                    .choices
                    .ok_or_else(|| EvalError::unsupported(UNSUPPORTED))?
                    .branch(items.len())
                    .map_err(|error| EvalError::unsupported(error.message))?;
                let item = *items
                    .get(choice)
                    .ok_or_else(|| EvalError::unsupported(UNSUPPORTED))?;
                self.predicate(scope, item, depth + 1)
            }
            Expr::If { cond, then, other } if self.source.is_some() => {
                let condition = ev.eval_scoped(scope, *cond, &self.st())?;
                let Some(condition) = condition.as_bool() else {
                    return Err(EvalError::unsupported(
                        "independent random Next condition is not Boolean",
                    ));
                };
                self.predicate(scope, if condition { *then } else { *other }, depth + 1)
            }
            Expr::Unchanged(body) if self.source.is_some() => {
                self.unchanged(scope, *body, depth + 1)
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
            | Expr::Builtin {
                op: BuiltinOp::In, ..
            } => Err(EvalError::unsupported(UNSUPPORTED)),
            _ => self.filter(scope, expr),
        }
    }

    fn equality(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        left: ExprId,
        right: ExprId,
    ) -> Result<bool, EvalError> {
        let Some(var) = self.variable(scope, left, 0)? else {
            return self.filter(scope, expr);
        };
        let ev = &self.model.evaluator;
        let value = ev.eval_scoped(scope, right, &self.st())?;
        if let Some(previous) = &self.slots[var.index()] {
            ev.equal_values(previous, &value)
        } else {
            self.slots[var.index()] = Some(value);
            Ok(true)
        }
    }

    fn filter(&self, scope: &Scope, expr: ExprId) -> Result<bool, EvalError> {
        let st = self.st();
        match self.model.evaluator.eval_scoped(scope, expr, &st) {
            Ok(Value::Bool(value)) => Ok(value),
            Ok(value) => Err(EvalError::unsupported(format!(
                "{UNSUPPORTED}: a filter returned {}",
                value.kind_name()
            ))),
            Err(error) => Err(error),
        }
    }
}
