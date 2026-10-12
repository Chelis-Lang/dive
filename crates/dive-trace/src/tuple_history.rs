//! Private whole-model tuple invariants for encounter-order admission.
//!
//! No supplied state, successor enumerator or successful prefix is evidence.
//! Every candidate has an Init obligation and an all-branches Next obligation.

use std::collections::BTreeSet;

use dive_eval::Scope;
use dive_ir::{Arg, BuiltinOp, Expr, ExprId, JunctionKind, OpRef, QuantKind, StdOp, VarId};
use dive_model::Model;

/// Sealed proof: only this module constructs source-owned shape hypotheses.
pub(crate) struct History {
    facts: Facts,
}

#[derive(Default, PartialEq, Eq)]
struct Facts {
    tuples: BTreeSet<VarId>,
    ranges: BTreeSet<VarId>,
}
#[derive(Clone, Copy)]
enum Shape {
    Tuple,
    Range,
}
impl Facts {
    fn contains(&self, shape: Shape, var: VarId) -> bool {
        match shape {
            Shape::Tuple => self.tuples.contains(&var),
            Shape::Range => self.ranges.contains(&var),
        }
    }
    fn insert(&mut self, shape: Shape, var: VarId) {
        match shape {
            Shape::Tuple => {
                self.tuples.insert(var);
            }
            Shape::Range => {
                self.ranges.insert(var);
            }
        }
    }
}
impl History {
    pub(crate) fn prove(model: &Model) -> Self {
        let mut proof = Proof {
            model,
            fuel: 32_768,
        };
        let empty = Facts::default();
        let mut facts = Facts::default();
        // Existing bounds are shared by both shape families. No submitted
        // state or successful checking prefix is evidence for these facts.
        if model.ir.vars().len() <= 256 {
            for index in 0..model.ir.vars().len() {
                let Ok(index) = u32::try_from(index) else {
                    break;
                };
                let var = VarId(index);
                for shape in [Shape::Tuple, Shape::Range] {
                    if model.init.iter().any(|init| {
                        proof.obligation(
                            init.pred.expr,
                            &init.pred.scope,
                            var,
                            false,
                            shape,
                            &empty,
                            0,
                        )
                    }) {
                        facts.insert(shape, var);
                    }
                }
            }
        }
        if let Some(next) = &model.next {
            loop {
                let mut kept = Facts::default();
                for (shape, variables) in
                    [(Shape::Tuple, &facts.tuples), (Shape::Range, &facts.ranges)]
                {
                    for var in variables {
                        if proof.obligation(next.expr, &next.scope, *var, true, shape, &facts, 0) {
                            kept.insert(shape, *var);
                        }
                    }
                }
                if kept == facts {
                    break;
                }
                facts = kept;
                if facts.tuples.is_empty() && facts.ranges.is_empty() {
                    break;
                }
            }
        }
        Self { facts }
    }

    pub(crate) fn native(&self, model: &Model, op: StdOp, args: &[Arg], scope: &Scope) -> bool {
        crate::exists::native_with_histories(
            model,
            op,
            args,
            scope,
            &self.facts.tuples,
            &self.facts.ranges,
        )
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
        shape: Shape,
        hypotheses: &Facts,
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
                    && self.shape(args[1], scope, shape, hypotheses, depth + 1))
                    || (self.target(args[1], var, next)
                        && self.shape(args[0], scope, shape, hypotheses, depth + 1))
            }
            Expr::Junction { kind, items, .. } => match kind {
                JunctionKind::And => items
                    .iter()
                    .any(|id| self.obligation(*id, scope, var, next, shape, hypotheses, depth + 1)),
                JunctionKind::Or => {
                    !items.is_empty()
                        && items.iter().all(|id| {
                            self.obligation(*id, scope, var, next, shape, hypotheses, depth + 1)
                        })
                }
            },
            Expr::If { then, other, .. } => {
                self.obligation(*then, scope, var, next, shape, hypotheses, depth + 1)
                    && self.obligation(*other, scope, var, next, shape, hypotheses, depth + 1)
            }
            Expr::Unchanged(body) if next && hypotheses.contains(shape, var) => {
                self.projection(*body, scope, var, depth + 1)
            }
            // An existential action's body must preserve the fact for any
            // satisfying binding. A universal over an empty domain does not
            // imply its body and is intentionally absent from this rule.
            Expr::Quant {
                kind: QuantKind::Exists,
                bounds,
                body,
            } if next
                && !bounds.is_empty()
                && bounds.iter().all(|bound| {
                    bound
                        .domain
                        .is_some_and(|domain| self.finite(domain, scope, depth + 1))
                }) =>
            {
                self.model
                    .evaluator
                    .scope_binder_analysis(scope, expr)
                    .is_ok_and(|entered| {
                        self.obligation(*body, &entered, var, next, shape, hypotheses, depth + 1)
                    })
            }
            Expr::Def(id) => self.definition(*id, &[], scope, var, next, shape, hypotheses, depth),
            Expr::Apply {
                callee: OpRef::Def(id),
                args,
            } => self.definition(*id, args, scope, var, next, shape, hypotheses, depth),
            Expr::Let { body, .. } => {
                self.model
                    .evaluator
                    .scope_let(scope, expr)
                    .is_ok_and(|entered| {
                        self.obligation(*body, &entered, var, next, shape, hypotheses, depth + 1)
                    })
            }
            Expr::Subst { body, bindings } => {
                crate::context_admission::substitution_is_lazy(bindings)
                    && self
                        .model
                        .evaluator
                        .scope_subst(scope, expr)
                        .is_ok_and(|entered| {
                            self.obligation(
                                *body,
                                &entered,
                                var,
                                next,
                                shape,
                                hypotheses,
                                depth + 1,
                            )
                        })
            }
            _ => false,
        }
    }

    fn shape(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        shape: Shape,
        facts: &Facts,
        depth: usize,
    ) -> bool {
        if self.fuel == 0 || depth > 64 {
            return false;
        }
        self.fuel -= 1;
        if matches!(shape, Shape::Tuple) {
            return crate::exists::tuple_with_histories(
                self.model,
                expr,
                scope,
                &facts.tuples,
                &facts.ranges,
            );
        }
        let ev = &self.model.evaluator;
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            Expr::Var { var, primed: false } => facts.ranges.contains(var),
            Expr::FcnCons { bounds, body } => {
                !bounds.is_empty()
                    && bounds.iter().all(|bound| {
                        bound
                            .domain
                            .is_some_and(|domain| self.finite(domain, scope, depth + 1))
                    })
                    && ev.scope_binder_analysis(scope, expr).is_ok_and(|entered| {
                        crate::exists::tuple_with_histories(
                            self.model,
                            *body,
                            &entered,
                            &facts.tuples,
                            &facts.ranges,
                        )
                    })
            }
            Expr::Except { base, updates } => {
                self.shape(*base, scope, shape, facts, depth + 1)
                    && updates.len() <= self.fuel
                    && updates.iter().all(|update| {
                        if self.fuel == 0 || update.path.len() != 1 {
                            return false;
                        }
                        self.fuel -= 1;
                        crate::exists::tuple_with_histories(
                            self.model,
                            update.value,
                            scope,
                            &facts.tuples,
                            &facts.ranges,
                        )
                    })
            }
            Expr::If { then, other, .. } => {
                self.shape(*then, scope, shape, facts, depth + 1)
                    && self.shape(*other, scope, shape, facts, depth + 1)
            }
            Expr::Local(slot) => match ev.scope_slot(scope, *slot) {
                dive_eval::SlotView::Lazy(body, entered) => {
                    self.shape(body, &entered, shape, facts, depth + 1)
                }
                _ => false,
            },
            Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(scope, *decl) {
                dive_eval::SlotView::Lazy(body, entered) => {
                    self.shape(body, &entered, shape, facts, depth + 1)
                }
                _ => false,
            },
            Expr::Def(id) => self.shape_definition(*id, &[], scope, shape, facts, depth),
            Expr::Apply {
                callee: OpRef::Def(id),
                args,
            } => self.shape_definition(*id, args, scope, shape, facts, depth),
            Expr::Let { body, .. } => ev
                .scope_let(scope, expr)
                .is_ok_and(|entered| self.shape(*body, &entered, shape, facts, depth + 1)),
            Expr::Subst { body, bindings } => {
                crate::context_admission::substitution_is_lazy(bindings)
                    && ev
                        .scope_subst(scope, expr)
                        .is_ok_and(|entered| self.shape(*body, &entered, shape, facts, depth + 1))
            }
            _ => false,
        }
    }

    fn shape_definition(
        &mut self,
        id: dive_ir::DefId,
        args: &[Arg],
        scope: &Scope,
        shape: Shape,
        facts: &Facts,
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
            .is_ok_and(|entered| self.shape(def.body, &entered, shape, facts, depth + 1))
    }

    // Closed source finite domains only. No enumeration, normalization,
    // forcing of opaque providers, sampled quantifier values or callbacks.
    fn finite(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> bool {
        if self.fuel == 0 || depth > 64 {
            return false;
        }
        self.fuel -= 1;
        let ev = &self.model.evaluator;
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            Expr::SetEnum(_) => true,
            Expr::If { then, other, .. } => {
                self.finite(*then, scope, depth + 1) && self.finite(*other, scope, depth + 1)
            }
            Expr::Const(id) => match ev.constant_binding(*id) {
                Some(dive_eval::ConstBinding::Value(dive_value::Value::Set(set))) => {
                    set.stored_elements().is_some()
                }
                Some(dive_eval::ConstBinding::Def(id)) => {
                    self.finite_definition(id, &[], scope, depth)
                }
                _ => false,
            },
            Expr::Def(id) => self.finite_definition(*id, &[], scope, depth),
            Expr::Apply {
                callee: OpRef::Def(id),
                args,
            } => self.finite_definition(*id, args, scope, depth),
            Expr::Local(slot) => match ev.scope_slot(scope, *slot) {
                dive_eval::SlotView::Value(dive_value::Value::Set(set)) => {
                    set.stored_elements().is_some()
                }
                dive_eval::SlotView::Lazy(body, entered) => self.finite(body, &entered, depth + 1),
                _ => false,
            },
            Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(scope, *decl) {
                dive_eval::SlotView::Value(dive_value::Value::Set(set)) => {
                    set.stored_elements().is_some()
                }
                dive_eval::SlotView::Lazy(body, entered) => self.finite(body, &entered, depth + 1),
                _ => false,
            },
            Expr::Let { body, .. } => ev
                .scope_let(scope, expr)
                .is_ok_and(|entered| self.finite(*body, &entered, depth + 1)),
            Expr::Subst { body, bindings } => {
                crate::context_admission::substitution_is_lazy(bindings)
                    && ev
                        .scope_subst(scope, expr)
                        .is_ok_and(|entered| self.finite(*body, &entered, depth + 1))
            }
            _ => false,
        }
    }

    fn finite_definition(
        &mut self,
        id: dive_ir::DefId,
        args: &[Arg],
        scope: &Scope,
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
            .is_ok_and(|entered| self.finite(def.body, &entered, depth + 1))
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
            Expr::Subst { body, bindings } => {
                crate::context_admission::substitution_is_lazy(bindings)
                    && ev
                        .scope_subst(scope, expr)
                        .is_ok_and(|entered| self.projection(*body, &entered, var, depth + 1))
            }
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
        shape: Shape,
        hypotheses: &Facts,
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
            self.obligation(def.body, &entered, var, next, shape, hypotheses, depth + 1)
        })
    }
}
