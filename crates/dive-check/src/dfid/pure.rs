//! Conservative context-aware purity proof for the staged DFID backend.
//!
//! IR effects alone cannot describe a public caller's captured operator slots.
//! Follow lazy/context bindings without evaluating them; opaque captures refuse.

use dive_eval::{ConstBinding, Scope, SlotView};
use dive_ir::{Arg, Effects, Expr, ExprId, OpRef};
use dive_model::{Model, Pred};
use dive_value::Value;

pub(super) fn predicate(model: &Model, pred: &Pred) -> bool {
    let mut budget = 50_000usize;
    expression(model, pred.expr, &pred.scope, 0, &mut budget)
}

fn slot(model: &Model, slot: SlotView, depth: u32, budget: &mut usize) -> bool {
    match slot {
        SlotView::Unset | SlotView::AnalysisBound => true,
        SlotView::Value(value) => material(&value),
        SlotView::Lazy(expr, scope) => expression(model, expr, &scope, depth + 1, budget),
        // Operator arguments and opaque lazy functions carry hidden environments.
        SlotView::Other => false,
    }
}

fn material(value: &Value) -> bool {
    // A function or lazy set can conceal a closure with captured effects. This
    // slice does not inspect those environments and never forces them to guess.
    !matches!(value, Value::Fcn(_) | Value::Set(_))
}

fn constant(
    model: &Model,
    constant: dive_ir::ConstId,
    scope: &Scope,
    args: &[Arg],
    depth: u32,
    budget: &mut usize,
) -> bool {
    match model.evaluator.constant_binding(constant) {
        Some(ConstBinding::Value(value)) => material(&value),
        Some(ConstBinding::Def(def)) => definition(model, def, scope, args, depth + 1, budget),
        None => false,
    }
}

fn definition(
    model: &Model,
    def: dive_ir::DefId,
    scope: &Scope,
    args: &[Arg],
    depth: u32,
    budget: &mut usize,
) -> bool {
    if depth > 128 || *budget == 0 {
        return false;
    }
    let def = model.evaluator.redirected(def);
    // Value overrides are opaque to this context proof.
    if model.evaluator.is_value_override(def) {
        return false;
    }
    let Some(node) = model.ir.defs().get(def.index()) else {
        return false;
    };
    if let Some(op) = node.std {
        return !op.impure()
            && !matches!(
                op.support(),
                dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
            )
            && args
                .iter()
                .all(|arg| argument(model, arg, scope, depth + 1, budget));
    }
    let called = if args.is_empty() {
        model.evaluator.scope_enter(scope, def)
    } else {
        model.evaluator.scope_apply(scope, def, args)
    };
    called.is_ok_and(|called| expression(model, node.body, &called, depth + 1, budget))
}

fn argument(model: &Model, arg: &Arg, scope: &Scope, depth: u32, budget: &mut usize) -> bool {
    match arg {
        Arg::Expr { expr, .. } => expression(model, *expr, scope, depth + 1, budget),
        // Opaque operators passed through a context are not proven pure merely
        // because their caller's static IR node has no IMPURE bit.
        Arg::Op { .. } => false,
    }
}

fn expression(model: &Model, expr: ExprId, scope: &Scope, depth: u32, budget: &mut usize) -> bool {
    if depth > 128 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    let forbidden = Effects::IMPURE
        .union(Effects::OPEN)
        .union(Effects::LIVE_TLCGET);
    let Some(node) = model.ir.nodes().get(expr.index()) else {
        return false;
    };
    if node.effects.intersects(forbidden) {
        return false;
    }
    match model.ir.expr(expr) {
        Expr::Local(local) => slot(
            model,
            model.evaluator.scope_slot(scope, *local),
            depth,
            budget,
        ),
        Expr::SubstRef { decl, .. } => {
            let bound = model.evaluator.scope_subst_slot(scope, *decl);
            if matches!(bound, SlotView::Unset) {
                model.ir.decls().get(decl.index()).is_some_and(|d| {
                    d.constant
                        .is_none_or(|c| constant(model, c, scope, &[], depth, budget))
                })
            } else {
                slot(model, bound, depth, budget)
            }
        }
        Expr::Const(c) => constant(model, *c, scope, &[], depth, budget),
        Expr::Def(def) => definition(model, *def, scope, &[], depth, budget),
        Expr::Apply { callee, args } => match callee {
            OpRef::Def(def) => definition(model, *def, scope, args, depth, budget),
            OpRef::Const(c) => constant(model, *c, scope, args, depth, budget),
            OpRef::Param(_) | OpRef::Subst(_) | OpRef::Unbound(_) => false,
            OpRef::Std(op) => {
                !op.impure()
                    && !matches!(
                        op.support(),
                        dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                    )
                    && args
                        .iter()
                        .all(|a| argument(model, a, scope, depth, budget))
            }
            OpRef::Builtin(_) => args
                .iter()
                .all(|a| argument(model, a, scope, depth, budget)),
        },
        _ => model
            .ir
            .expr(expr)
            .children()
            .iter()
            .all(|child| expression(model, *child, scope, depth + 1, budget)),
    }
}
