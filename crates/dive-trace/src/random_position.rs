//! A conservative certificate for random-stream alignment.
//!
//! This recognizes deterministic assignments behind unprimed guards. It
//! does not enumerate successors or construct states. A single enabled
//! branch, assigning every variable once, consumes random draws in the same
//! order when generated and when evaluated as a predicate. Quantifiers,
//! primed guards, shared effects and multiple enabled branches are refused.

use std::collections::BTreeSet;

use dive_eval::{ConstBinding, Scope, St};
use dive_ir::{Arg, BuiltinOp, Effects, Expr, ExprId, JunctionKind, Level, OpRef, StdOp};
use dive_model::{Model, Pred};
use dive_value::Value;

/// Whether an initial predicate or next-state relation has a proved draw
/// position. `None` means the proof is unavailable, never that it is false.
pub(crate) fn aligned(model: &Model, pred: &Pred, st: &St, initial: bool) -> bool {
    if model.evaluator.scope_shares_side_effects(&pred.scope) {
        return false;
    }
    let mut proof = Proof {
        model,
        st,
        initial,
        fuel: 256,
    };
    if initial
        && model.ir.defs().iter().any(|d| {
            d.std.is_none()
                && d.params.is_empty()
                && d.level == Level::Constant
                && (d.effects.contains(Effects::IMPURE)
                    || (matches!(d.scope, dive_ir::DefScope::TopLevel)
                        && !proof.bound_pure(d.body)))
        })
    {
        // Top-level constant definitions and assumptions can advance the startup
        // stream before Init. Configured calls can hide those draws from
        // source effect metadata; no fresh-stream certificate is then made.
        // A nested zero-arity LET captures its enclosing operator's parameters;
        // it is not independently evaluated during startup. In particular,
        // Sequences.SelectSeq's local F is not an opaque startup operation.
        return false;
    }
    proof
        .branches(pred.expr, &pred.scope)
        .is_some_and(|branches| {
            let mut enabled = branches.into_iter().filter(|b| b.enabled);
            enabled
                .next()
                .is_some_and(|b| b.assigned.len() == model.ir.vars().len())
                && enabled.next().is_none()
        })
}

#[derive(Default)]
struct Branch {
    enabled: bool,
    assigned: BTreeSet<usize>,
}

struct Proof<'a> {
    model: &'a Model,
    st: &'a St,
    initial: bool,
    fuel: usize,
}

impl Proof<'_> {
    fn branches(&mut self, expr: ExprId, scope: &Scope) -> Option<Vec<Branch>> {
        self.fuel = self.fuel.checked_sub(1)?;
        match self.model.ir.expr(expr) {
            Expr::Def(def) => {
                let def = self.model.evaluator.redirected(*def);
                let d = self.model.ir.def(def);
                if d.std.is_some() || !d.params.is_empty() || d.recursive {
                    return None;
                }
                let entered = self.model.evaluator.scope_enter(scope, def).ok()?;
                self.branches(d.body, &entered)
            }
            Expr::Junction {
                kind: JunctionKind::Or,
                items,
                ..
            } => {
                let mut out = Vec::new();
                for item in items {
                    out.extend(self.branches(*item, scope)?);
                }
                Some(out)
            }
            _ => {
                let mut branch = Branch {
                    enabled: true,
                    ..Branch::default()
                };
                self.conjunct(expr, scope, &mut branch)?;
                Some(vec![branch])
            }
        }
    }

    fn conjunct(&mut self, expr: ExprId, scope: &Scope, branch: &mut Branch) -> Option<()> {
        if !branch.enabled {
            return Some(());
        }
        self.fuel = self.fuel.checked_sub(1)?;
        let m = self.model;
        match m.ir.expr(expr) {
            Expr::Def(def) => {
                let def = m.evaluator.redirected(*def);
                let d = m.ir.def(def);
                if d.std.is_some() || !d.params.is_empty() || d.recursive {
                    return None;
                }
                let entered = m.evaluator.scope_enter(scope, def).ok()?;
                self.conjunct(d.body, &entered, branch)
            }
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for item in items {
                    self.conjunct(*item, scope, branch)?;
                }
                Some(())
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                if let Expr::Var { var, primed } = m.ir.expr(args[0])
                    && *primed != self.initial
                {
                    // A scalar assignment with a pure RHS or one direct
                    // random operation on pure arguments. No RHS may read
                    // a successor variable.
                    if !self.rhs(args[1]) || !branch.assigned.insert(var.index()) {
                        return None;
                    }
                    return Some(());
                }
                self.guard(expr, scope, branch)
            }
            _ => self.guard(expr, scope, branch),
        }
    }

    fn guard(&self, expr: ExprId, scope: &Scope, branch: &mut Branch) -> Option<()> {
        let level = if self.initial {
            Level::Constant
        } else {
            Level::State
        };
        if !self.pure(expr, level) {
            return None;
        }
        let pred = Pred::new(expr, scope.clone());
        match self.model.eval(&pred, self.st).ok()? {
            Value::Bool(true) => Some(()),
            Value::Bool(false) if branch.assigned.is_empty() => {
                // This branch is excluded before any assignment or draw.
                branch.enabled = false;
                Some(())
            }
            _ => None,
        }
    }

    fn pure(&self, expr: ExprId, level: Level) -> bool {
        let node = self.model.ir.node(expr);
        node.level <= level
            && !node.effects.contains(Effects::IMPURE)
            && !node.effects.contains(Effects::USES_ENABLED)
            && !node.effects.contains(Effects::OPEN)
            && !node.effects.contains(Effects::TEMPORAL)
            && self.bound_pure(expr)
    }

    // Source effects cannot certify a body after configuration redirects it
    // to native randomness. This bounded, non-evaluating walk rejects actual
    // native effects and opaque operator parameters, even when public typed
    // IR supplied a pure original body. None is never interpreted as purity.
    fn bound_pure(&self, expr: ExprId) -> bool {
        let mut work = vec![expr];
        let mut seen = BTreeSet::new();
        let mut fuel = 256_usize;
        while let Some(expr) = work.pop() {
            let Some(remaining) = fuel.checked_sub(1) else {
                return false;
            };
            fuel = remaining;
            if !seen.insert(expr) {
                continue;
            }
            let Some(node) = self.model.ir.nodes().get(expr.index()) else {
                return false;
            };
            if matches!(&node.expr, Expr::Apply { args, .. } if args.len() > fuel)
                || matches!(&node.expr, Expr::Subst { bindings, .. } if bindings.len() > fuel)
            {
                return false;
            }
            let mut ops = node.expr.op_refs().into_iter().cloned().collect::<Vec<_>>();
            match &node.expr {
                Expr::Def(id) => ops.push(OpRef::Def(*id)),
                Expr::Const(id) => ops.push(OpRef::Const(*id)),
                _ => {}
            }
            for op in ops {
                let def = match op {
                    OpRef::Def(id) => Some(id),
                    OpRef::Const(id) => match self.model.evaluator.constant_binding(id) {
                        Some(ConstBinding::Def(id)) => Some(id),
                        Some(ConstBinding::Value(_)) => None,
                        None => return false,
                    },
                    OpRef::Std(op) if op.impure() => return false,
                    OpRef::Std(_) | OpRef::Builtin(_) => None,
                    OpRef::Param(_) | OpRef::Subst(_) | OpRef::Unbound(_) => return false,
                };
                if let Some(def) = def {
                    let def = self.model.evaluator.redirected(def);
                    let Some(def) = self.model.ir.defs().get(def.index()) else {
                        return false;
                    };
                    match def.std {
                        Some(op) if op.impure() => return false,
                        Some(_) => {}
                        None => work.push(def.body),
                    }
                }
            }
            let children = node.expr.children();
            if work
                .len()
                .checked_add(children.len())
                .is_none_or(|n| n > fuel)
            {
                return false;
            }
            work.extend(children);
        }
        true
    }

    fn native(&self, op: &OpRef) -> Option<StdOp> {
        let def = match op {
            OpRef::Std(op) => return Some(*op),
            OpRef::Def(id) => *id,
            OpRef::Const(id) => match self.model.evaluator.constant_binding(*id)? {
                ConstBinding::Def(id) => id,
                ConstBinding::Value(_) => return None,
            },
            _ => return None,
        };
        self.model
            .ir
            .defs()
            .get(self.model.evaluator.redirected(def).index())?
            .std
    }

    fn rhs(&self, expr: ExprId) -> bool {
        let level = if self.initial {
            Level::Constant
        } else {
            Level::State
        };
        if self.pure(expr, level) {
            return true;
        }
        let Expr::Apply { callee, args } = self.model.ir.expr(expr) else {
            return false;
        };
        let op = self.native(callee);
        op.is_some_and(|op| {
            matches!(
                op,
                StdOp::TlcRandomElement
                    | StdOp::RandomizationRandomSubset
                    | StdOp::RandomizationRandomSetOfSubsets
                    | StdOp::RandomizationRandomSubsetSet
            )
        }) && args
            .iter()
            .all(|a| matches!(a, Arg::Expr { expr, .. } if self.pure(*expr, level)))
    }
}
