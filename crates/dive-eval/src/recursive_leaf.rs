//! State-level recursive operators, which temporal planning and validation
//! treat as evaluation leaves, as TLC does.
//!
//! Temporal planning and the trace validator inline every non-recursive
//! operator they meet, so that the temporal structure and each binder of a
//! temporal atom is seen before evaluation. A `RECURSIVE` operator cannot be
//! inlined: its unfolding depends on the actual arguments. TLC does not expand
//! a call of a `RECURSIVE` operator whose level is at most `ActionLevel`; it
//! leaves the call to `Tool.eval`, which handles the recursion
//! (`Liveness.astToLive`, `Liveness.java:196-245` at the pinned oracle).
//!
//! dive admits the state-level part of that rule: a top-level `RECURSIVE`
//! operator whose body, together with every definition it reaches, is at most
//! state level and contains no operator argument, `ENABLED`, dynamic
//! substitution or effect.
//! State-level binders, `LET` and recursive functions are ordinary values of
//! that body. Admission uses semantic levels and effects, not expression shape.
//! Its arguments are still analysed in the caller's context; the
//! call itself is a leaf that the core evaluator computes. Action-level
//! recursive operators keep the named refusal.

use std::collections::BTreeSet;

use dive_ir::{Arg, DefId, DefScope, Effects, Expr, ExprId, Level, OpRef};

use crate::{ConstBinding, Evaluator};

impl Evaluator {
    /// Whether a call of `def`, a top-level `RECURSIVE` operator, is a pure
    /// state-level evaluation leaf: its body and every definition it reaches
    /// are at most state level, with no operator argument, `ENABLED`,
    /// dynamic substitution or effect. Value-bound constants are supported;
    /// configured operator bindings and overrides retain their refusal. `false`
    /// for anything else, including a non-recursive or
    /// overridden definition.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use dive_eval::{Evaluator, Model, Options};
    /// let source = "---- MODULE M ----\nEXTENDS Naturals, Sequences\nVARIABLE v\n\
    ///     RECURSIVE Sum(_)\nSum(q) == IF q = <<>> THEN 0 ELSE Head(q) + Sum(Tail(q))\n\
    ///     RECURSIVE Any(_)\nAny(q) == IF q = <<>> THEN FALSE ELSE \\E x \\in {Head(q)} : x > 0 \\/ Any(Tail(q))\n\
    ///     RECURSIVE Next(_)\nNext(q) == IF q = <<>> THEN v' = v ELSE Next(Tail(q))\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    /// let ir = dive_ir::lower(&analysis).expect("IR");
    /// let (sum, any, next) = (
    ///     ir.root_def("Sum").expect("Sum"),
    ///     ir.root_def("Any").expect("Any"),
    ///     ir.root_def("Next").expect("Next"),
    /// );
    /// let evaluator = Evaluator::new(Model::new(Arc::new(ir)), Options::tlc()).expect("context");
    /// assert!(evaluator.recursive_leaf(sum));
    /// assert!(evaluator.recursive_leaf(any));
    /// assert!(!evaluator.recursive_leaf(next));
    /// ```
    #[must_use]
    pub fn recursive_leaf(&self, def: DefId) -> bool {
        // Both definition and expression edges use a work list: cyclic
        // recursion and long acyclic helper chains cannot consume Rust stack.
        enum Work {
            Definition(DefId),
            Expression(ExprId),
        }
        let ir = self.ir();
        let Some(info) = ir.defs().get(def.index()) else {
            return false;
        };
        if !info.recursive || info.scope != DefScope::TopLevel || info.std.is_some() {
            return false;
        }
        let mut seen_defs = BTreeSet::new();
        let mut seen_exprs = BTreeSet::new();
        let mut todo = vec![Work::Definition(def)];
        while let Some(work) = todo.pop() {
            let expr = match work {
                Work::Definition(def) => {
                    if !seen_defs.insert(def) {
                        continue;
                    }
                    let Some(info) = ir.defs().get(def.index()) else {
                        return false;
                    };
                    if self.is_value_override(def) || self.redirected(def) != def {
                        return false;
                    }
                    // Native definitions use their evaluator implementation,
                    // not the TLA+ body retained for documentation.
                    if let Some(op) = info.std {
                        if !pure_native(op) {
                            return false;
                        }
                        continue;
                    }
                    if info.level > Level::State
                        || info.params.iter().any(|p| p.arity != 0)
                        || info
                            .effects
                            .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
                    {
                        return false;
                    }
                    info.body
                }
                Work::Expression(expr) => expr,
            };
            if !seen_exprs.insert(expr) {
                continue;
            }
            let Some(node) = ir.nodes().get(expr.index()) else {
                return false;
            };
            if node.level > Level::State
                || node
                    .effects
                    .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
            {
                return false;
            }
            match &node.expr {
                Expr::Const(constant) => {
                    if !matches!(
                        self.constant_binding(*constant),
                        Some(ConstBinding::Value(_))
                    ) {
                        return false;
                    }
                }
                Expr::Def(def) => todo.push(Work::Definition(*def)),
                Expr::Apply { callee, args } => {
                    if args.iter().any(|arg| matches!(arg, Arg::Op { .. })) {
                        return false;
                    }
                    match callee {
                        OpRef::Def(def) => todo.push(Work::Definition(*def)),
                        OpRef::Std(op) if pure_native(*op) => {}
                        _ => return false,
                    }
                }
                Expr::Let { defs, .. } => {
                    // Inspect even unused local definitions. Their lexical
                    // scope is part of this closed body's proof.
                    todo.extend(defs.iter().copied().map(Work::Definition));
                }
                // A substituted declaration can obtain a different level or
                // effect from its dynamic context. Static metadata cannot
                // establish the leaf proof for that context.
                Expr::Subst { .. } | Expr::SubstRef { .. } => return false,
                // Other expressions use the core evaluator unchanged. Their
                // semantic level/effects and all children are checked above
                // and below, including binders and lazy function bodies.
                _ => {}
            }
            node.expr
                .for_each_child(|child| todo.push(Work::Expression(child)));
        }
        true
    }
}

/// A standard-module operator that computes a value only: no effect, I/O or
/// checker introspection.
fn pure_native(op: dive_ir::StdOp) -> bool {
    !op.impure()
        && !matches!(
            op.support(),
            dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
        )
}
