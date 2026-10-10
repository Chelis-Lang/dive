//! Concrete finite-binding guards for already proved pure predicates.

use crate::{ConstBinding, EvalError, Evaluator, Scope, SlotView, St, StateRef};
use dive_ir::{Arg, BuiltinOp, Effects, Expr, ExprId, OpRef, StdOp};

impl Evaluator {
    /// Checks finite constructor/quantifier bindings in the actual state or
    /// edge before interpreting a pure predicate. Domains are never frozen
    /// during planning. Each binder is limited to `limit` values/bindings;
    /// nested walks also have depth and total-visit bounds. These are entry
    /// bounds, not a byte-memory or general evaluation-time guarantee.
    ///
    /// The caller must separately establish scoped purity and valid levels.
    /// This walk rejects effects and opaque operators before reading domains,
    /// and never calls the successor enumerator. It does not evaluate the
    /// predicate's truth or certify successor existence.
    ///
    /// # Errors
    /// Missing, infinite, opaque or oversized bindings; missing evaluation
    /// context; effects; or an explicit traversal limit.
    pub fn check_finite_bindings(
        &self,
        scope: &Scope,
        expr: ExprId,
        st: &St,
        limit: usize,
    ) -> crate::Result<()> {
        self.validate_scope(scope)?;
        self.admit_scoped_expr(scope, expr)?;
        Guard {
            evaluator: self,
            limit,
            visits: 0,
        }
        .walk(scope, expr, st, 0)
    }
}

struct Guard<'a> {
    evaluator: &'a Evaluator,
    limit: usize,
    visits: usize,
}
impl Guard<'_> {
    fn shifted(st: &St) -> St {
        match &st.s1 {
            StateRef::Vals(after) => St::state(after.clone()),
            StateRef::Empty if matches!(st.s0, StateRef::Empty) => St::constant(),
            // An absent successor may still be harmless (e.g. UNCHANGED x
            // while checking ENABLED). Any domain that actually reads it must
            // fail evaluation; no placeholder value is supplied.
            _ => St::empty(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit lexical binding and priming rules"
    )]
    fn walk(&mut self, scope: &Scope, expr: ExprId, st: &St, depth: usize) -> crate::Result<()> {
        self.visits = self.visits.saturating_add(1);
        if depth > 256 || self.visits > self.limit.saturating_mul(64).min(100_000) {
            return Err(EvalError::resource_limit("finite binding traversal"));
        }
        let ev = self.evaluator;
        ev.admit_expr(expr)?;
        let node = ev.ir().node(expr);
        if node
            .effects
            .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
            || ev.scope_shares_side_effects(scope)
        {
            return Err(EvalError::unsupported(
                "effects in finite binding preflight",
            ));
        }
        match &node.expr {
            Expr::Prime(body) => return self.walk(scope, *body, &Self::shifted(st), depth + 1),
            Expr::Unchanged(body) => {
                self.walk(scope, *body, st, depth + 1)?;
                return self.walk(scope, *body, &Self::shifted(st), depth + 1);
            }
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                self.walk(scope, *action, st, depth + 1)?;
                self.walk(scope, *sub, st, depth + 1)?;
                return self.walk(scope, *sub, &Self::shifted(st), depth + 1);
            }
            Expr::Builtin {
                op: BuiltinOp::Prime | BuiltinOp::Unchanged,
                args,
            } => {
                for body in args {
                    if matches!(
                        node.expr,
                        Expr::Builtin {
                            op: BuiltinOp::Unchanged,
                            ..
                        }
                    ) {
                        self.walk(scope, *body, st, depth + 1)?;
                    }
                    self.walk(scope, *body, &Self::shifted(st), depth + 1)?;
                }
                return Ok(());
            }
            Expr::Let { body, .. } => {
                return self.walk(&ev.scope_let(scope, expr)?, *body, st, depth + 1);
            }
            Expr::Subst { body, .. } => {
                return self.walk(&ev.scope_subst(scope, expr)?, *body, st, depth + 1);
            }
            Expr::Local(local) => {
                return match ev.scope_slot(scope, *local) {
                    SlotView::Value(_) => Ok(()),
                    SlotView::Lazy(body, sc) => self.walk(&sc, body, st, depth + 1),
                    SlotView::AnalysisBound => Err(EvalError::unsupported(
                        "analysis-only binder in runtime preflight",
                    )),
                    _ => Err(EvalError::unsupported("opaque finite binding context")),
                };
            }
            Expr::SubstRef { decl, primed } => {
                let context = if *primed {
                    Self::shifted(st)
                } else {
                    st.clone()
                };
                return match ev.scope_subst_slot(scope, *decl) {
                    SlotView::Value(_) => Ok(()),
                    SlotView::Lazy(body, sc) => self.walk(&sc, body, &context, depth + 1),
                    _ => Err(EvalError::unsupported("opaque substituted finite binding")),
                };
            }
            Expr::Const(c) => {
                return match ev.constant_binding(*c) {
                    Some(ConstBinding::Value(_)) => Ok(()),
                    Some(ConstBinding::Def(d)) => {
                        let d = ev.redirected(d);
                        self.walk(
                            &ev.scope_enter(scope, d)?,
                            ev.ir().def(d).body,
                            st,
                            depth + 1,
                        )
                    }
                    None => Err(EvalError::unsupported("unbound finite binding constant")),
                };
            }
            Expr::Quant { bounds, body, .. }
            | Expr::FcnCons { bounds, body, .. }
            | Expr::SetMap { bounds, expr: body } => {
                for bound in bounds {
                    let dom = bound
                        .domain
                        .ok_or_else(|| EvalError::unsupported("unbounded predicate domain"))?;
                    self.walk(scope, dom, st, depth + 1)?;
                }
                for sc in ev.scope_bindings_bounded(scope, expr, st, self.limit)? {
                    self.walk(&sc, *body, st, depth + 1)?;
                }
                return Ok(());
            }
            Expr::SetFilter { bound, pred } => {
                let dom = bound
                    .domain
                    .ok_or_else(|| EvalError::unsupported("unbounded predicate domain"))?;
                self.walk(scope, dom, st, depth + 1)?;
                for sc in ev.scope_bindings_bounded(scope, expr, st, self.limit)? {
                    self.walk(&sc, *pred, st, depth + 1)?;
                }
                return Ok(());
            }
            Expr::Apply {
                callee: OpRef::Builtin(BuiltinOp::Prime | BuiltinOp::Unchanged),
                args,
            } => {
                for arg in args {
                    let Arg::Expr { expr: body, .. } = arg else {
                        return Err(EvalError::unsupported(
                            "operator argument in primed finite binding",
                        ));
                    };
                    if matches!(
                        node.expr,
                        Expr::Apply {
                            callee: OpRef::Builtin(BuiltinOp::Unchanged),
                            ..
                        }
                    ) {
                        self.walk(scope, *body, st, depth + 1)?;
                    }
                    self.walk(scope, *body, &Self::shifted(st), depth + 1)?;
                }
                return Ok(());
            }
            _ => {}
        }
        let call = match &node.expr {
            Expr::Def(d) => Some((OpRef::Def(*d), &[][..])),
            Expr::Apply { callee, args } => Some((callee.clone(), args.as_ref())),
            _ => None,
        };
        if let Some((op, args)) = call {
            let standard = match op {
                OpRef::Def(d) => {
                    let d = ev.redirected(d);
                    let def = ev.ir().def(d);
                    if def.recursive || args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                        return Err(EvalError::unsupported(
                            "recursive/higher-order finite binding",
                        ));
                    }
                    if !ev.is_value_override(d) && def.std.is_none() {
                        return self.walk(
                            &ev.scope_apply(scope, d, args)?,
                            def.body,
                            st,
                            depth + 1,
                        );
                    }
                    def.std
                }
                OpRef::Std(s) => Some(s),
                OpRef::Param(_) | OpRef::Const(_) | OpRef::Subst(_) => {
                    return Err(EvalError::unsupported("dynamic finite binding operator"));
                }
                _ => None,
            };
            if standard.is_some_and(|s| s.impure() || s == StdOp::TlcTLCGet) {
                return Err(EvalError::unsupported(
                    "effectful/checker finite binding operator",
                ));
            }
        }
        for child in node.expr.children() {
            self.walk(scope, child, st, depth + 1)?;
        }
        Ok(())
    }
}
