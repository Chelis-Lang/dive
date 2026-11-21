//! Admission for replay with a host that refuses actual diagnostic output.

use dive_ir::{Arg, Expr, ExprId, OpRef, StdOp};
use dive_model::Model;

/// Proves that every operation in an action is first-order and its only
/// possible output effects are direct Print/PrintT calls. Inspect pure children
/// too: a configured definition or operator actual may hide a different effect.
pub(crate) fn closed_action(model: &Model, root: ExprId, scope: &dive_eval::Scope) -> bool {
    if scope.has_substitutions() || model.evaluator.scope_has_effects(scope) {
        return false;
    }
    let mut pending = vec![root];
    let mut visited = std::collections::HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let expr = model.ir.expr(id);
        match expr {
            Expr::Apply {
                callee: OpRef::Std(op),
                args,
            } if args.iter().all(|arg| matches!(arg, Arg::Expr { .. }))
                && op.kind() == dive_ir::StdKind::Value
                && op.support() == dive_ir::StdSupport::Core =>
            {
                if op.impure() && !matches!(op, StdOp::TlcPrint | StdOp::TlcPrintT) {
                    return false;
                }
            }
            Expr::Bool(_)
            | Expr::Num(_)
            | Expr::Str(_)
            | Expr::Var { .. }
            | Expr::Builtin { .. }
            | Expr::Junction { .. }
            | Expr::Not(_)
            | Expr::Implies(..)
            | Expr::Equiv(..)
            | Expr::If { .. }
            | Expr::SetEnum(_)
            | Expr::Tuple(_)
            | Expr::Unchanged(_) => {}
            _ => return false,
        }
        expr.for_each_child(|child| pending.push(child));
    }
    true
}
