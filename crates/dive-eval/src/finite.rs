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
            analysis: false,
        }
        .walk(scope, expr, st, 0)
    }
}

struct Guard<'a> {
    evaluator: &'a Evaluator,
    limit: usize,
    visits: usize,
    analysis: bool,
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

    fn operator_context(
        &mut self,
        scope: &Scope,
        operator: &OpRef,
        st: &St,
        depth: usize,
    ) -> crate::Result<()> {
        let ceiling = self.limit.saturating_mul(64).min(100_000);
        let mut remaining = ceiling.saturating_sub(self.visits);
        let native = self
            .evaluator
            .scope_native_operator_analysis(scope, operator, &mut remaining);
        self.visits = ceiling - remaining;
        let (def, analysis) = if native?.is_some() {
            (None, scope.clone())
        } else {
            let (def, analysis) = self
                .evaluator
                .scope_operator_analysis(scope, operator)?
                .ok_or_else(|| {
                    EvalError::unsupported("opaque/recursive/higher-order finite operator")
                })?;
            (Some(def), analysis)
        };
        let saved = self.analysis;
        self.analysis = true;
        let result = (|| {
            let ceiling = self.limit.saturating_mul(64).min(100_000);
            let mut remaining = ceiling.saturating_sub(self.visits);
            let captures = self
                .evaluator
                .scope_operator_captures(&analysis, &mut remaining);
            self.visits = ceiling - remaining;
            let captures = captures?
                .ok_or_else(|| EvalError::unsupported("opaque finite operator context"))?;
            for (body, captured) in captures {
                self.walk(&captured, body, st, depth + 1)?;
            }
            if let Some(def) = def {
                self.walk(&analysis, self.evaluator.ir().def(def).body, st, depth + 1)?;
            }
            Ok(())
        })();
        self.analysis = saved;
        result
    }

    // Newly admitted native formals may read the caller's states or cached
    // lexical values. Source purity is insufficient to admit a deferred
    // provider. Inspect storage, names and order before native evaluation.
    #[allow(
        clippy::too_many_lines,
        reason = "explicit bounded callback-free representation/context admission walk"
    )]
    fn native_values(&mut self, scope: &Scope, st: &St) -> crate::Result<()> {
        use crate::env::{Dyn, Ev, Frame, OpVal, Sv};
        use std::sync::Arc;
        enum Work {
            Frame(Arc<Frame>, usize),
            Dynamic(Dyn, usize),
            Slot(Sv, usize),
            Value(dive_value::Value, usize),
        }
        let cap = self.limit.saturating_mul(64).min(50_000);
        let mut work = vec![
            Work::Frame(scope.env.frame.clone(), 0),
            Work::Dynamic(scope.env.dynamic.clone(), 0),
        ];
        for state in [&st.s0, &st.s1] {
            if let StateRef::Vals(values) = state {
                if work.len().saturating_add(values.len()) > cap.saturating_sub(self.visits) {
                    return Err(EvalError::resource_limit("native stored input work"));
                }
                work.extend(values.iter().flatten().cloned().map(|v| Work::Value(v, 0)));
            }
        }
        while let Some(item) = work.pop() {
            self.visits = self.visits.saturating_add(1);
            if self.visits > cap {
                return Err(EvalError::resource_limit("native stored input work"));
            }
            let depth = match &item {
                Work::Frame(_, d) | Work::Dynamic(_, d) | Work::Slot(_, d) | Work::Value(_, d) => {
                    *d
                }
            };
            if depth > 128 {
                return Err(EvalError::resource_limit("native stored input depth"));
            }
            let room = |pending: usize, count: usize| -> crate::Result<()> {
                if pending.saturating_add(count) > cap.saturating_sub(self.visits) {
                    Err(EvalError::resource_limit("native stored input work"))
                } else {
                    Ok(())
                }
            };
            match item {
                Work::Frame(frame, _) => {
                    room(
                        work.len(),
                        frame
                            .slots
                            .len()
                            .saturating_add(usize::from(frame.parent.is_some())),
                    )?;
                    work.extend(
                        frame
                            .slots
                            .iter()
                            .cloned()
                            .map(|v| Work::Slot(v, depth + 1)),
                    );
                    if let Some(parent) = &frame.parent {
                        work.push(Work::Frame(parent.clone(), depth + 1));
                    }
                }
                Work::Dynamic(dynamic, _) => {
                    if dynamic.at.is_some() {
                        return Err(EvalError::unsupported("native captured @ context"));
                    }
                    let mut link = dynamic.subst;
                    while let Some(binding) = link.take() {
                        self.visits = self.visits.saturating_add(1);
                        if self.visits >= cap || work.len() >= cap.saturating_sub(self.visits) {
                            return Err(EvalError::resource_limit("native stored input work"));
                        }
                        work.push(Work::Slot(binding.val.clone(), depth + 1));
                        link.clone_from(&binding.next);
                    }
                }
                Work::Slot(Sv::Val(v), _) => {
                    room(work.len(), 1)?;
                    work.push(Work::Value(v, depth + 1));
                }
                Work::Slot(Sv::Thunk(thunk), _) => {
                    room(work.len(), 3)?;
                    work.push(Work::Frame(thunk.env.frame.clone(), depth + 1));
                    work.push(Work::Dynamic(thunk.env.dynamic.clone(), depth + 1));
                    let cached = thunk
                        .cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(cached) = cached.as_ref() {
                        let Ev::V(v) = &cached.val else {
                            return Err(EvalError::unsupported("native cached opaque function"));
                        };
                        let count = [&cached.s0, &cached.s1]
                            .iter()
                            .map(|s| match s {
                                StateRef::Vals(v) => v.len(),
                                _ => 0,
                            })
                            .fold(1_usize, usize::saturating_add);
                        room(work.len(), count)?;
                        work.push(Work::Value(v.clone(), depth + 1));
                        for state in [&cached.s0, &cached.s1] {
                            if let StateRef::Vals(values) = state {
                                work.extend(
                                    values
                                        .iter()
                                        .flatten()
                                        .cloned()
                                        .map(|v| Work::Value(v, depth + 1)),
                                );
                            }
                        }
                    }
                }
                Work::Slot(
                    Sv::Op(OpVal::Def {
                        parent, dynamic, ..
                    }),
                    _,
                ) => {
                    room(
                        work.len(),
                        usize::from(parent.is_some()) + usize::from(dynamic.is_some()),
                    )?;
                    if let Some(parent) = parent {
                        work.push(Work::Frame(parent, depth + 1));
                    }
                    if let Some(dynamic) = dynamic {
                        work.push(Work::Dynamic(dynamic, depth + 1));
                    }
                }
                Work::Slot(Sv::Fcn(_), _) => {
                    return Err(EvalError::unsupported("native captured opaque function"));
                }
                Work::Slot(_, _) => {}
                Work::Value(value, _) => match value {
                    dive_value::Value::Str(atom) | dive_value::Value::Model(atom) => {
                        if self
                            .evaluator
                            .core
                            .names
                            .resolve(atom.tok())
                            .is_none_or(|owned| owned.as_str() != atom.as_str())
                        {
                            return Err(EvalError::unsupported("native stored input namespace"));
                        }
                    }
                    dive_value::Value::Fcn(function) => {
                        if function.kind() != dive_value::FcnKind::Tuple
                            && function.order() != self.evaluator.options().ctx.order
                        {
                            return Err(EvalError::unsupported("native stored input order"));
                        }
                        room(work.len(), function.len().saturating_mul(2))?;
                        for (key, value) in function.iter() {
                            work.push(Work::Value(key, depth + 1));
                            work.push(Work::Value(value.clone(), depth + 1));
                        }
                    }
                    dive_value::Value::Set(set) => {
                        if set.order() != self.evaluator.options().ctx.order {
                            return Err(EvalError::unsupported("native stored input order"));
                        }
                        let Some(elements) = set.stored_elements() else {
                            return Err(EvalError::unsupported("native opaque/deferred input set"));
                        };
                        room(work.len(), elements.len())?;
                        work.extend(elements.iter().cloned().map(|v| Work::Value(v, depth + 1)));
                    }
                    _ => {}
                },
            }
        }
        Ok(())
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
                    SlotView::AnalysisBound if self.analysis => Ok(()),
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
                if self.analysis {
                    self.walk(
                        &ev.scope_binder_analysis(scope, expr)?,
                        *body,
                        st,
                        depth + 1,
                    )?;
                } else {
                    for sc in ev.scope_bindings_bounded(scope, expr, st, self.limit)? {
                        self.walk(&sc, *body, st, depth + 1)?;
                    }
                }
                return Ok(());
            }
            Expr::SetFilter { bound, pred } => {
                let dom = bound
                    .domain
                    .ok_or_else(|| EvalError::unsupported("unbounded predicate domain"))?;
                self.walk(scope, dom, st, depth + 1)?;
                if self.analysis {
                    self.walk(
                        &ev.scope_binder_analysis(scope, expr)?,
                        *pred,
                        st,
                        depth + 1,
                    )?;
                } else {
                    for sc in ev.scope_bindings_bounded(scope, expr, st, self.limit)? {
                        self.walk(&sc, *pred, st, depth + 1)?;
                    }
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
                    if def.recursive {
                        return Err(EvalError::unsupported(
                            "recursive/higher-order finite binding",
                        ));
                    }
                    if !ev.is_value_override(d) && def.std.is_none() {
                        for arg in args {
                            if let Arg::Op { op, .. } = arg {
                                self.operator_context(scope, op, st, depth + 1)?;
                            }
                        }
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
                OpRef::Param(local) => {
                    let ceiling = self.limit.saturating_mul(64).min(100_000);
                    let mut remaining = ceiling.saturating_sub(self.visits);
                    let native = ev.scope_native_operator_analysis(
                        scope,
                        &OpRef::Param(local),
                        &mut remaining,
                    );
                    self.visits = ceiling - remaining;
                    let native = native?;
                    if native
                        .is_some_and(|op| i32::try_from(args.len()).ok() != Some(op.java_arity()))
                    {
                        return Err(EvalError::unsupported(
                            "native finite binding argument arity",
                        ));
                    }
                    self.operator_context(scope, &OpRef::Param(local), st, depth + 1)?;
                    for arg in args {
                        let Arg::Expr { expr, .. } = arg else {
                            return Err(EvalError::unsupported(
                                "higher-order finite binding actual",
                            ));
                        };
                        self.walk(scope, *expr, st, depth + 1)?;
                    }
                    if native.is_some() {
                        if !self.analysis {
                            self.native_values(scope, st)?;
                        }
                        return Ok(());
                    }
                    let (def, called) =
                        ev.scope_apply_param(scope, local, args).ok_or_else(|| {
                            EvalError::unsupported("opaque finite binding operator")
                        })??;
                    return self.walk(&called, ev.ir().def(def).body, st, depth + 1);
                }
                OpRef::Const(_) | OpRef::Subst(_) => {
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
