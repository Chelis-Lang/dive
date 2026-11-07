//! Side effects that remain after start-up.
//!
//! The binder evaluates each zero-arity constant-level definition once at
//! start-up and keeps its value, as TLC's `SpecProcessor.processConstantDefns`
//! does; a configured `Op = v` likewise replaces a definition by a value.
//! Checking then reads the kept value and never evaluates the body again, so
//! a `RandomElement` in such a body draws once, from the run's start-up
//! stream (reported with its seed, D-002), and never while checking. The IR's
//! syntactic [`Effects::IMPURE`] flag cannot see this: it propagates through
//! every reference. [`Evaluator::impure_after_startup`] answers the same
//! question with the fixed definitions as values, and is conservative in
//! every case it does not resolve.

use dive_ir::{Arg, DefId, Effects, Expr, ExprId, OpRef, StdOp};

use crate::Evaluator;

impl Evaluator {
    /// Whether start-up fixed the value of `def` for the whole run: it was
    /// precomputed, or the configuration assigns it a value, and it is not
    /// redirected to another definition.
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, d: dive_ir::DefId) {
    /// if ev.startup_fixed(d) {
    ///     assert!(ev.precomputed(d).is_some() || ev.is_value_override(d));
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn startup_fixed(&self, def: DefId) -> bool {
        self.redirected(def) == def
            && (self.precomputed(def).is_some() || self.is_value_override(def))
    }

    /// Whether evaluating `expr` while checking can have a side effect
    /// (output, registers, randomness, time, I/O or checker state): its
    /// IR [`Effects::IMPURE`] flag, except that a definition
    /// [`Self::startup_fixed`] is a value. Every source the walk does not
    /// resolve (operator parameters, instance substitutions, `LET` and
    /// parameter reads, an unknown node) counts as impure.
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, e: dive_ir::ExprId) {
    /// let ir = ev.ir();
    /// if ev.impure_after_startup(e) {
    ///     assert!(ir.node(e).effects.intersects(dive_ir::Effects::IMPURE));
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn impure_after_startup(&self, expr: ExprId) -> bool {
        let ir = self.ir();
        let impure = |e: ExprId| {
            ir.nodes()
                .get(e.index())
                .is_none_or(|n| n.effects.intersects(Effects::IMPURE))
        };
        if !impure(expr) {
            return false;
        }
        let mut seen_defs = vec![false; ir.defs().len()];
        let mut seen = vec![false; ir.nodes().len()];
        let mut work = vec![expr];
        // A definition the walk reaches: its body, and its redirect target.
        let mut reach = |d: DefId, work: &mut Vec<ExprId>| -> bool {
            for d in [d, self.redirected(d)] {
                if self.startup_fixed(d) {
                    continue;
                }
                let Some(slot) = seen_defs.get_mut(d.index()) else {
                    return true;
                };
                if *slot {
                    continue;
                }
                *slot = true;
                let Some(def) = ir.defs().get(d.index()) else {
                    return true;
                };
                if !def.effects.intersects(Effects::IMPURE) {
                    continue;
                }
                if def.std.is_some() {
                    return true;
                }
                work.push(def.body);
            }
            false
        };
        while let Some(e) = work.pop() {
            match seen.get_mut(e.index()) {
                Some(s) if !*s => *s = true,
                Some(_) => continue,
                None => return true,
            }
            if !impure(e) {
                continue;
            }
            let node = ir.node(e);
            match &node.expr {
                Expr::Def(d) => {
                    if reach(*d, &mut work) {
                        return true;
                    }
                }
                Expr::Local(_) | Expr::SubstRef { .. } | Expr::Subst { .. } => return true,
                Expr::Apply { callee, args } => {
                    let key = args.first().and_then(|a| match a {
                        Arg::Expr { expr, .. } => Some(ir.expr(*expr)),
                        Arg::Op { .. } => None,
                    });
                    if opref_impure(callee, key, &mut work, &mut reach) {
                        return true;
                    }
                    for a in args {
                        if let Arg::Op { op, .. } = a
                            && opref_impure(op, None, &mut work, &mut reach)
                        {
                            return true;
                        }
                    }
                }
                Expr::Let { defs, .. } => {
                    for d in defs {
                        if reach(*d, &mut work) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
            node.expr.for_each_child(|c| work.push(c));
        }
        false
    }
}

/// Whether an operator reference is itself an impure source; a definition
/// is walked instead.
fn opref_impure(
    op: &OpRef,
    key: Option<&Expr>,
    work: &mut Vec<ExprId>,
    reach: &mut impl FnMut(DefId, &mut Vec<ExprId>) -> bool,
) -> bool {
    match op {
        OpRef::Def(d) => reach(*d, work),
        OpRef::Std(StdOp::TlcTLCGet) => match key {
            Some(Expr::Str(k)) => dive_ir::effects::tlcget_string_is_live(k),
            _ => true,
        },
        OpRef::Std(op) => op.impure(),
        // Operator parameters and substitutions carry the effects of any
        // operator passed as an argument anywhere.
        OpRef::Param(_) | OpRef::Subst(_) => true,
        OpRef::Builtin(_) | OpRef::Const(_) | OpRef::Unbound(_) => false,
    }
}
