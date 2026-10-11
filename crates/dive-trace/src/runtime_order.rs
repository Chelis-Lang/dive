//! Independent, unevaluated admission of tuple-only name-capable calls.
//!
//! This inspects bound checking roles, never states supplied by a witness.
//! Unknown operand shapes retain the existing runtime-order refusal.

use dive_eval::{ConstBinding, Scope, SlotView};
use dive_ir::{Arg, DefId, Expr, ExprId, OpRef, StdOp};
use dive_model::Model;
use dive_value::Value;

/// Whether the existing runtime-order guard can be discharged independently.
pub(crate) fn admitted(model: &Model) -> bool {
    if !model.runtime_name_order {
        return true;
    }
    if model.validate_context().is_err() {
        return false;
    }
    let mut proof = Proof { model, fuel: 8192 };
    for pred in model
        .init
        .iter()
        .map(|init| &init.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|named| &named.pred))
        .chain(model.implied_inits.iter().map(|named| &named.pred))
        .chain(model.implied_actions.iter().map(|named| &named.pred))
        .chain(model.constraints.iter().map(|named| &named.pred))
        .chain(model.action_constraints.iter().map(|named| &named.pred))
        .chain(model.temporals.iter().map(|named| &named.pred))
        .chain(model.implied_temporals.iter().map(|named| &named.pred))
        .chain(model.configured_properties.iter().map(|named| &named.pred))
        .chain(model.view.iter().map(|named| &named.pred))
    {
        if !proof.walk(pred.expr, &pred.scope, 0) {
            return false;
        }
    }
    for &id in &model.assumptions {
        let Ok(scope) = model.evaluator.scope(id) else {
            return false;
        };
        if !proof.walk(model.ir.def(id).body, &scope, 0) {
            return false;
        }
    }
    true
}

struct Proof<'a> {
    model: &'a Model,
    fuel: usize,
}

/// A deliberately closed list: purity alone does not prove that an override
/// cannot construct names (for example, Json's serializers return strings).
fn known_nonconstructing(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::NaturalsNat
            | StdOp::NaturalsPlus
            | StdOp::NaturalsMinus
            | StdOp::NaturalsTimes
            | StdOp::NaturalsLt
            | StdOp::NaturalsLeq
            | StdOp::NaturalsGt
            | StdOp::NaturalsGeq
            | StdOp::NaturalsDotDot
            | StdOp::NaturalsDiv
            | StdOp::NaturalsMod
            | StdOp::NaturalsExpt
            | StdOp::IntegersInt
            | StdOp::IntegersNat
            | StdOp::IntegersPlus
            | StdOp::IntegersMinus
            | StdOp::IntegersTimes
            | StdOp::IntegersLt
            | StdOp::IntegersLeq
            | StdOp::IntegersGt
            | StdOp::IntegersGeq
            | StdOp::IntegersDotDot
            | StdOp::IntegersNeg
            | StdOp::IntegersDiv
            | StdOp::IntegersMod
            | StdOp::IntegersExpt
            | StdOp::FiniteSetsIsFiniteSet
            | StdOp::FiniteSetsCardinality
            | StdOp::SequencesLen
            | StdOp::SequencesHead
            | StdOp::SequencesCons
            | StdOp::SequencesAppend
    )
}

impl Proof<'_> {
    fn charge(&mut self, depth: usize) -> bool {
        if depth > 64 || self.fuel == 0 {
            return false;
        }
        self.fuel -= 1;
        true
    }

    /// Inspect stored values without forcing a provider or normalizing a set.
    fn value(&mut self, value: &Value, depth: usize) -> bool {
        if !self.charge(depth) {
            return false;
        }
        match value {
            Value::Set(set) => set.stored_elements().is_some_and(|elements| {
                elements.len() <= self.fuel
                    && elements.iter().all(|value| self.value(value, depth + 1))
            }),
            Value::Fcn(function) => {
                function.len() <= self.fuel / 2
                    && function.iter().all(|(domain, value)| {
                        self.value(&domain, depth + 1) && self.value(value, depth + 1)
                    })
            }
            _ => true,
        }
    }

    fn slot(&mut self, slot: SlotView, depth: usize) -> bool {
        match slot {
            SlotView::Value(value) => self.value(&value, depth),
            SlotView::Lazy(expr, scope) => self.walk(expr, &scope, depth),
            SlotView::AnalysisBound => true,
            _ => false,
        }
    }

    fn call(&mut self, op: &OpRef, args: &[Arg], scope: &Scope, depth: usize) -> bool {
        if args.len() > self.fuel || args.iter().any(|arg| matches!(arg, Arg::Op { .. })) {
            return false;
        }
        let ev = &self.model.evaluator;
        let standard = match op {
            OpRef::Std(op) => Some(*op),
            OpRef::Def(id) => {
                let id = ev.redirected(*id);
                let Some(definition) = self.model.ir.defs().get(id.index()) else {
                    return false;
                };
                if definition.recursive || ev.is_value_override(id) {
                    return false;
                }
                if let Some(op) = definition.std {
                    Some(op)
                } else {
                    let Ok(called) = ev.scope_apply(scope, id, args) else {
                        return false;
                    };
                    if !self.walk(definition.body, &called, depth + 1) {
                        return false;
                    }
                    None
                }
            }
            OpRef::Builtin(_) => None,
            // Opaque operator arguments/substitutions require another proof.
            _ => return false,
        };
        if standard.is_some_and(|op| {
            !known_nonconstructing(op)
                && !crate::exists::native_tuple_only(self.model, op, args, scope)
        }) {
            return false;
        }
        args.iter().all(|arg| match arg {
            Arg::Expr { expr, .. } => self.walk(*expr, scope, depth + 1),
            Arg::Op { .. } => false,
        })
    }

    fn definition(&mut self, id: DefId, scope: &Scope, depth: usize) -> bool {
        self.call(&OpRef::Def(id), &[], scope, depth)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit non-evaluating scoped admission cases"
    )]
    fn walk(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> bool {
        if !self.charge(depth) {
            return false;
        }
        let ev = &self.model.evaluator;
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            Expr::Def(id) => return self.definition(*id, scope, depth),
            Expr::Apply { callee, args } => return self.call(callee, args, scope, depth),
            Expr::Local(slot) => return self.slot(ev.scope_slot(scope, *slot), depth + 1),
            Expr::SubstRef { decl, .. } => {
                return self.slot(ev.scope_subst_slot(scope, *decl), depth + 1);
            }
            Expr::Const(id) => {
                return match ev.constant_binding(*id) {
                    Some(ConstBinding::Value(value)) => self.value(&value, depth + 1),
                    Some(ConstBinding::Def(id)) => self.definition(id, scope, depth + 1),
                    None => false,
                };
            }
            Expr::Let { body, .. } => {
                return ev
                    .scope_let(scope, expr)
                    .is_ok_and(|entered| self.walk(*body, &entered, depth + 1));
            }
            Expr::Subst { body, .. } => {
                return ev
                    .scope_subst(scope, expr)
                    .is_ok_and(|entered| self.walk(*body, &entered, depth + 1));
            }
            Expr::Quant { bounds, body, .. }
            | Expr::FcnCons { bounds, body }
            | Expr::SetMap { bounds, expr: body } => {
                return self.binder(expr, bounds, *body, scope, depth);
            }
            Expr::SetFilter { bound, pred } => {
                return self.binder(expr, std::slice::from_ref(bound), *pred, scope, depth);
            }
            Expr::RecFcn { .. } => return false,
            _ => {}
        }
        let children = node.expr.children();
        children.len() <= self.fuel
            && children
                .iter()
                .all(|child| self.walk(*child, scope, depth + 1))
    }

    fn binder(
        &mut self,
        expr: ExprId,
        bounds: &[dive_ir::Bound],
        body: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> bool {
        if bounds.len() > self.fuel {
            return false;
        }
        for bound in bounds {
            if !bound
                .domain
                .is_some_and(|domain| self.walk(domain, scope, depth + 1))
            {
                return false;
            }
        }
        self.model
            .evaluator
            .scope_binder_analysis(scope, expr)
            .is_ok_and(|entered| self.walk(body, &entered, depth + 1))
    }
}
