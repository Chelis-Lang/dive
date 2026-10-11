//! Private whole-model tuple invariants for encounter-order admission.
//!
//! No supplied state, successor enumerator or successful prefix is evidence.
//! Every candidate has an Init obligation and an all-branches Next obligation.

use std::collections::BTreeSet;

use dive_eval::Scope;
use dive_ir::{Arg, BuiltinOp, Expr, ExprId, JunctionKind, OpRef, StdOp, VarId};
use dive_model::Model;

/// Sealed proof: only this module can construct its variable hypotheses.
pub(crate) struct History {
    variables: BTreeSet<VarId>,
}
impl History {
    pub(crate) fn prove(model: &Model) -> Self {
        let mut proof = Proof {
            model,
            fuel: 32_768,
        };
        let empty = BTreeSet::new();
        let mut variables = BTreeSet::new();
        // A deliberately bounded closed first slice. Larger models retain
        // literal tuple admission, without assuming variable shapes.
        if model.ir.vars().len() <= 256 {
            for index in 0..model.ir.vars().len() {
                let Ok(index) = u32::try_from(index) else {
                    break;
                };
                let var = VarId(index);
                if model.init.iter().any(|init| {
                    proof.obligation(init.pred.expr, &init.pred.scope, var, false, &empty, 0)
                }) {
                    variables.insert(var);
                }
            }
        }
        if let Some(next) = &model.next {
            loop {
                let kept = variables
                    .iter()
                    .copied()
                    .filter(|var| {
                        proof.obligation(next.expr, &next.scope, *var, true, &variables, 0)
                    })
                    .collect::<BTreeSet<_>>();
                if kept == variables {
                    break;
                }
                variables = kept;
                if variables.is_empty() {
                    break;
                }
            }
        }
        Self { variables }
    }

    pub(crate) fn native(&self, model: &Model, op: StdOp, args: &[Arg], scope: &Scope) -> bool {
        crate::exists::native_with_variables(model, op, args, scope, &self.variables)
    }
}

struct Proof<'a> {
    model: &'a Model,
    fuel: usize,
}
impl Proof<'_> {
    fn target(&self, expr: ExprId, var: VarId, primed: bool) -> bool {
        match self
            .model
            .ir
            .nodes()
            .get(expr.index())
            .map(|node| &node.expr)
        {
            Some(Expr::Var {
                var: actual,
                primed: actual_prime,
            }) => *actual == var && *actual_prime == primed,
            Some(Expr::Prime(body)) if primed => matches!(
                self.model.ir.nodes().get(body.index()).map(|node| &node.expr),
                Some(Expr::Var { var: actual, primed: false }) if *actual == var
            ),
            _ => false,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "explicit scoped proof obligation and inductive hypotheses"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "closed non-evaluating logical rules; no fallback evaluation"
    )]
    fn obligation(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        var: VarId,
        next: bool,
        hypotheses: &BTreeSet<VarId>,
        depth: usize,
    ) -> bool {
        if self.fuel == 0 || depth > 64 {
            return false;
        }
        self.fuel -= 1;
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                (self.target(args[0], var, next)
                    && crate::exists::tuple_with_variables(self.model, args[1], scope, hypotheses))
                    || (self.target(args[1], var, next)
                        && crate::exists::tuple_with_variables(
                            self.model, args[0], scope, hypotheses,
                        ))
            }
            Expr::Junction { kind, items, .. } => match kind {
                JunctionKind::And => items
                    .iter()
                    .any(|id| self.obligation(*id, scope, var, next, hypotheses, depth + 1)),
                JunctionKind::Or => {
                    !items.is_empty()
                        && items
                            .iter()
                            .all(|id| self.obligation(*id, scope, var, next, hypotheses, depth + 1))
                }
            },
            Expr::If { then, other, .. } => {
                self.obligation(*then, scope, var, next, hypotheses, depth + 1)
                    && self.obligation(*other, scope, var, next, hypotheses, depth + 1)
            }
            Expr::Unchanged(body) if next && hypotheses.contains(&var) => {
                self.projection(*body, scope, var, depth + 1)
            }
            Expr::Def(id) => self.definition(*id, &[], scope, var, next, hypotheses, depth),
            Expr::Apply {
                callee: OpRef::Def(id),
                args,
            } => self.definition(*id, args, scope, var, next, hypotheses, depth),
            Expr::Let { body, .. } => {
                self.model
                    .evaluator
                    .scope_let(scope, expr)
                    .is_ok_and(|entered| {
                        self.obligation(*body, &entered, var, next, hypotheses, depth + 1)
                    })
            }
            Expr::Subst { body, .. } => {
                self.model
                    .evaluator
                    .scope_subst(scope, expr)
                    .is_ok_and(|entered| {
                        self.obligation(*body, &entered, var, next, hypotheses, depth + 1)
                    })
            }
            _ => false,
        }
    }

    // Equality of a fixed tuple entails equality at each fixed projection.
    // A conditional/permutation can move a variable to another position, so
    // merely finding that variable in every branch is not sufficient.
    fn projection(&mut self, expr: ExprId, scope: &Scope, var: VarId, depth: usize) -> bool {
        if self.fuel == 0 || depth > 64 {
            return false;
        }
        self.fuel -= 1;
        let ev = &self.model.evaluator;
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            Expr::Var {
                var: actual,
                primed: false,
            } => *actual == var,
            Expr::Tuple(items) => items
                .iter()
                .any(|item| self.projection(*item, scope, var, depth + 1)),
            Expr::Def(id) => self.projection_definition(*id, &[], scope, var, depth),
            Expr::Apply {
                callee: OpRef::Def(id),
                args,
            } => self.projection_definition(*id, args, scope, var, depth),
            Expr::Local(slot) => match ev.scope_slot(scope, *slot) {
                dive_eval::SlotView::Lazy(body, entered) => {
                    self.projection(body, &entered, var, depth + 1)
                }
                _ => false,
            },
            Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(scope, *decl) {
                dive_eval::SlotView::Lazy(body, entered) => {
                    self.projection(body, &entered, var, depth + 1)
                }
                _ => false,
            },
            Expr::Let { body, .. } => ev
                .scope_let(scope, expr)
                .is_ok_and(|entered| self.projection(*body, &entered, var, depth + 1)),
            Expr::Subst { body, .. } => ev
                .scope_subst(scope, expr)
                .is_ok_and(|entered| self.projection(*body, &entered, var, depth + 1)),
            _ => false,
        }
    }

    fn projection_definition(
        &mut self,
        id: dive_ir::DefId,
        args: &[Arg],
        scope: &Scope,
        var: VarId,
        depth: usize,
    ) -> bool {
        let ev = &self.model.evaluator;
        let id = ev.redirected(id);
        let Some(def) = self.model.ir.defs().get(id.index()) else {
            return false;
        };
        if def.recursive
            || def.std.is_some()
            || ev.is_value_override(id)
            || args.iter().any(|arg| matches!(arg, Arg::Op { .. }))
        {
            return false;
        }
        ev.scope_apply(scope, id, args)
            .is_ok_and(|entered| self.projection(def.body, &entered, var, depth + 1))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "preserve the exact predicate obligation across a scoped call"
    )]
    fn definition(
        &mut self,
        id: dive_ir::DefId,
        args: &[Arg],
        scope: &Scope,
        var: VarId,
        next: bool,
        hypotheses: &BTreeSet<VarId>,
        depth: usize,
    ) -> bool {
        let ev = &self.model.evaluator;
        let id = ev.redirected(id);
        let Some(def) = self.model.ir.defs().get(id.index()) else {
            return false;
        };
        if def.recursive
            || def.std.is_some()
            || ev.is_value_override(id)
            || args.iter().any(|arg| matches!(arg, Arg::Op { .. }))
        {
            return false;
        }
        ev.scope_apply(scope, id, args).is_ok_and(|entered| {
            self.obligation(def.body, &entered, var, next, hypotheses, depth + 1)
        })
    }
}
