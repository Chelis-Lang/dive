//! Admit finite function equations whose recursive reads have strict predecessors.

use std::collections::HashSet;

use crate::{Arg, Bound, BuiltinOp, DefScope, Expr, ExprId, Ir, LocalRef, OpRef, Slot, StdOp};

/// A bound equation of the form `CHOOSE f : f = [i \in D |-> E]`.
/// Every use of the function in `E` is an application at `i - 1`.
#[derive(Clone, Copy, Debug)]
pub struct PrefixEquation<'a> {
    /// The function's slot in the current lexical frame.
    pub function: Slot,
    /// The one simple index binding, including its complete domain.
    pub index: &'a Bound,
    /// The function entry expression.
    pub entry: ExprId,
}

impl Ir {
    /// Recognizes the shape needed to construct a unique solution in ascending
    /// integer order. The evaluator must establish a finite integer domain and
    /// actually evaluate each demanded entry before publishing a value.
    #[must_use]
    pub fn prefix_equation(
        &self,
        bounds: &[Bound],
        predicate: ExprId,
    ) -> Option<PrefixEquation<'_>> {
        let [choice] = bounds else { return None };
        let [function] = choice.vars.as_ref() else {
            return None;
        };
        if choice.tuple || choice.domain.is_some() {
            return None;
        }
        let Expr::Builtin {
            op: BuiltinOp::Eq,
            args,
        } = self.expr(predicate)
        else {
            return None;
        };
        let [left, right] = args.as_ref() else {
            return None;
        };
        let local =
            |id| matches!(self.expr(id), Expr::Local(LocalRef { up: 0, slot }) if slot == function);
        let constructor = if local(*left) {
            *right
        } else if local(*right) {
            *left
        } else {
            return None;
        };
        let Expr::FcnCons { bounds, body } = self.expr(constructor) else {
            return None;
        };
        let [index] = bounds.as_ref() else {
            return None;
        };
        let [slot] = index.vars.as_ref() else {
            return None;
        };
        if index.tuple
            || !admitted(self, index.domain?, *function, None)
            || !admitted(self, *body, *function, Some(*slot))
        {
            return None;
        }
        Some(PrefixEquation {
            function: *function,
            index,
            entry: *body,
        })
    }
}

fn admits_callee(ir: &Ir, callee: &OpRef) -> bool {
    matches!(callee, OpRef::Std(_) | OpRef::Builtin(_))
        || matches!(callee, OpRef::Def(id) if ir.def(*id).scope == DefScope::TopLevel)
}

fn admitted(ir: &Ir, root: ExprId, function: Slot, index: Option<Slot>) -> bool {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        if seen.len() > 16_384 {
            return false;
        }
        let expr = ir.expr(id);
        match expr {
            Expr::Local(local) if local.slot == function => return false,
            Expr::FcnApply { fcn, arg } if matches!(ir.expr(*fcn), Expr::Local(LocalRef { up: 0, slot }) if *slot == function) =>
            {
                if !index.is_some_and(|slot| is_predecessor(ir, *arg, slot)) {
                    return false;
                }
                continue;
            }
            Expr::Let { defs, .. } => {
                for definition in defs.iter().map(|id| ir.def(*id)) {
                    if !matches!(definition.scope, DefScope::LetValue { .. })
                        || definition.recursive
                    {
                        return false;
                    }
                    pending.push(definition.body);
                }
            }
            Expr::Apply { callee, args }
                if !admits_callee(ir, callee)
                    || args.iter().any(|arg| matches!(arg, Arg::Op { .. })) =>
            {
                return false;
            }
            Expr::Def(id)
                if !matches!(
                    ir.def(*id).scope,
                    DefScope::TopLevel | DefScope::LetValue { .. }
                ) =>
            {
                return false;
            }
            Expr::Subst { .. } | Expr::SubstRef { .. } | Expr::RecFcn { .. } => return false,
            _ => {}
        }
        expr.for_each_child(|child| pending.push(child));
    }
    true
}

fn is_predecessor(ir: &Ir, expr: ExprId, index: Slot) -> bool {
    let Expr::Apply {
        callee: OpRef::Std(StdOp::NaturalsMinus | StdOp::IntegersMinus),
        args,
    } = ir.expr(expr)
    else {
        return false;
    };
    let [Arg::Expr { expr: lhs, .. }, Arg::Expr { expr: rhs, .. }] = args.as_ref() else {
        return false;
    };
    matches!(ir.expr(*lhs), Expr::Local(LocalRef { up: 0, slot }) if *slot == index)
        && matches!(ir.expr(*rhs), Expr::Num(value) if value.as_ref() == "1")
}
