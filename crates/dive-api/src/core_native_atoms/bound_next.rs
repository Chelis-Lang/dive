//! Binding identity for original split Next branches.
use dive_eval::{Scope, SlotView};
use dive_ir::{Arg, DefScope, Expr, ExprId, OpRef, actions::ActionName};
use dive_model::Model;
use dive_value::Value;

/// Match only a whole top-level definition with the same materialized arguments.
/// The result is the zero-based index supplied by the native successor producer.
/// This identifies a relation. Its diagnostics, nonstuttering subscript and coverage
/// by a particular source row require separate proofs.
pub(crate) fn action_index(model: &Model, expr: ExprId, scope: &Scope) -> Option<usize> {
    let ev = &model.evaluator;
    if !ev.admits_scoped_expr(scope, expr) || !scope.shares_pure_source_walk(scope, ev, expr) {
        return None;
    }
    let (def, args): (_, &[Arg]) = match model.ir.expr(expr) {
        Expr::Def(def) => (*def, &[]),
        Expr::Apply {
            callee: OpRef::Def(def),
            args,
        } => (*def, args),
        _ => return None,
    };
    let d = model.ir.def(def);
    if d.scope != DefScope::TopLevel
        || d.source.is_some()
        || d.std.is_some()
        || d.recursive
        || ev.redirected(def) != def
        || ev.is_value_override(def)
        || args.len() != d.params.len()
        || d.params.iter().any(|param| param.arity != 0)
    {
        return None;
    }
    let mut body = d.body;
    while let Expr::Let { body: inner, .. } | Expr::Label { body: inner, .. } = model.ir.expr(body)
    {
        body = *inner;
    }
    let actuals: Option<Vec<Value>> = args
        .iter()
        .map(|arg| {
            let Arg::Expr { expr, .. } = arg else {
                return None;
            };
            match model.ir.expr(*expr) {
                Expr::Local(local) => match ev.scope_slot(scope, *local) {
                    SlotView::Value(value) => Some(value),
                    _ => None,
                },
                Expr::Bool(b) => Some(Value::Bool(*b)),
                Expr::Str(s) => model.names.get(s).map(Value::Str),
                Expr::Const(c) => match ev.constant_binding(*c)? {
                    dive_eval::ConstBinding::Value(v) => Some(v),
                    dive_eval::ConstBinding::Def(_) => None,
                },
                Expr::Num(n) => n.parse().ok().map(Value::Int),
                _ => None,
            }
        })
        .collect();
    let actuals = actuals?;
    let mut matches = model.actions.iter().enumerate().filter(|(_, branch)| {
        if branch.name() != ActionName::Def(def)
            || branch.expr() != body
            || branch.params().len() != actuals.len()
        {
            return false;
        }
        let captured = branch.scope();
        captured.def() == def
            && captured.shares_pure_source_walk(&captured, ev, body)
            && branch
                .params()
                .iter()
                .zip(&actuals)
                .all(|((_, a), b)| same_actual(a, b))
    });
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}

/// Bound names are typed. Restrict structural comparison to small flat
/// records of scalar values; no lazy set expansion or receiver conversion.
fn same_actual(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Fcn(a), Value::Fcn(b)) => {
            a.ptr_eq(b)
                || a.as_record().zip(b.as_record()).is_some_and(|(a, b)| {
                    a.len() == b.len()
                        && a.len() <= 16
                        && a.iter()
                            .zip(b)
                            .all(|((k, x), (l, y))| k == l && same_scalar(x, y))
                })
        }
        _ => same_scalar(a, b),
    }
}

fn same_scalar(a: &Value, b: &Value) -> bool {
    matches!(
        a,
        Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Model(_)
    ) && std::mem::discriminant(a) == std::mem::discriminant(b)
        && a == b
}
