//! A conservative certificate for random-stream alignment.
//!
//! This recognizes deterministic assignments behind unprimed guards. It
//! does not enumerate successors or construct states. A single enabled
//! branch, assigning every variable once, consumes random draws in the same
//! order when generated and when evaluated as a predicate. Quantifiers,
//! primed guards, shared effects and multiple enabled branches are refused.

use std::collections::BTreeSet;

use dive_eval::{Scope, St};
use dive_ir::{Arg, BuiltinOp, Effects, Expr, ExprId, JunctionKind, Level, OpRef, StdOp};
use dive_model::{Model, Pred};
use dive_value::Value;

/// Whether an initial predicate or next-state relation has a proved draw
/// position. `None` means the proof is unavailable, never that it is false.
pub(crate) fn aligned(model: &Model, pred: &Pred, st: &St, initial: bool) -> bool {
    if model.evaluator.scope_shares_side_effects(&pred.scope) {
        return false;
    }
    if initial
        && model.ir.defs().iter().any(|d| {
            d.std.is_none() && d.level == Level::Constant && d.effects.contains(Effects::IMPURE)
        })
    {
        // Constant definitions and assumptions can advance the startup
        // stream before Init; a fresh stream is then insufficient.
        return false;
    }
    let mut proof = Proof {
        model,
        st,
        initial,
        fuel: 256,
    };
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
        let op = match callee {
            OpRef::Std(op) => Some(*op),
            OpRef::Def(def) => self.model.ir.def(self.model.evaluator.redirected(*def)).std,
            _ => None,
        };
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
