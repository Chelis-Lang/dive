//! Concrete finite-binding guards for already proved pure predicates.

use crate::{ConstBinding, EvalError, Evaluator, Scope, SlotView, St, StateRef};
use dive_ir::{Arg, BuiltinOp, ConstId, DefScope, Effects, Expr, ExprId, Level, OpRef, StdOp};

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
            selectseq: false,

            current_sequence: None,
        }
        .walk(scope, expr, st, 0)
    }

    /// Inspects an authoritative `SelectSeq` call without evaluating its sequence
    /// or predicate. Returns the original sequence and callback/capture bodies
    /// in their lexical scopes. Only binder-free unary user callbacks qualify.
    /// Callers independently check purity and scoped levels on every body.
    ///
    /// # Errors
    /// Invalid ownership, unsupported callback contexts or exhausted work.
    #[allow(
        clippy::type_complexity,
        reason = "original sequence plus lexical body projections; no opaque certificate"
    )]
    pub fn scope_selectseq_call(
        &self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        work: &mut usize,
    ) -> crate::Result<Option<(ExprId, Vec<(ExprId, Scope)>)>> {
        self.validate_scope(scope)?;
        let standard = match operator {
            OpRef::Std(s) => Some(*s),
            OpRef::Def(d) => {
                self.admit_def(*d)?;
                if self.redirected(*d) != *d || self.is_value_override(*d) {
                    return Ok(None);
                }
                self.ir().def(*d).std
            }
            _ => None,
        };
        if standard != Some(StdOp::SequencesSelectSeq) {
            return Ok(None);
        }
        if self
            .core
            .std_redirects
            .contains_key(&StdOp::SequencesSelectSeq)
        {
            return Err(EvalError::unsupported(
                "SelectSeq configured standard target",
            ));
        }
        let [Arg::Expr { expr: sequence, .. }, Arg::Op { op, .. }] = args else {
            return Err(EvalError::unsupported(
                "SelectSeq sequence/operator argument shape",
            ));
        };
        // Validate the reference before looking up the value. The existing
        // resolver performs all definition/lexical/configured-origin checks.
        *work = work
            .checked_sub(self.core.consts.len().saturating_add(1))
            .ok_or_else(|| EvalError::resource_limit("SelectSeq reference work"))?;
        let Some(_) = self.scope_operator_analysis(scope, op)? else {
            return Err(EvalError::unsupported("SelectSeq user callback target"));
        };
        let value = self.core.op_value(&scope.env, op)?;
        let (_, bodies) = self.selectseq_context(scope, value, None, None, work)?;
        Ok(Some((*sequence, bodies)))
    }

    pub(crate) fn selectseq_context(
        &self,
        scope: &Scope,
        value: crate::env::OpVal,
        element: Option<&dive_value::Value>,
        current_sequence: Option<&dive_value::Fcn>,
        work: &mut usize,
    ) -> crate::Result<(Scope, Vec<(ExprId, Scope)>)> {
        let (def, mut called) = self
            .selectseq_value_analysis(scope, value, work)?
            .ok_or_else(|| EvalError::unsupported("SelectSeq user callback target"))?;
        if self.ir().def(def).params.len() != 1 {
            return Err(EvalError::unsupported("SelectSeq unary value callback"));
        }
        if let Some(element) = element {
            called.env.bind(0, crate::env::Sv::Val(element.clone()));
        }
        let mut bodies = self
            .operator_captures(&called, work, true)?
            .ok_or_else(|| EvalError::unsupported("SelectSeq opaque/dynamic callback capture"))?;
        *work = work
            .checked_sub(1)
            .ok_or_else(|| EvalError::resource_limit("SelectSeq body work"))?;
        bodies.push((self.ir().def(def).body, called.clone()));
        let initial = *work;
        let mut guard = Guard {
            evaluator: self,
            limit: initial.div_ceil(64),
            visits: 0,
            analysis: true,
            selectseq: true,

            current_sequence: current_sequence.cloned(),
        };
        let result: crate::Result<()> = (|| {
            // Actual stored slots, including unused captures, are inspected.
            // Bounded execution bypasses thunk result caches;
            // no provider is normalized or iterated here.
            guard.native_values(&called, &St::constant())?;
            for (body, captured) in &bodies {
                guard.walk(captured, *body, &St::constant(), 0)?;
            }
            Ok(())
        })();
        *work = initial.saturating_sub(guard.visits);
        if guard.visits > initial {
            return Err(EvalError::resource_limit("SelectSeq callback work"));
        }
        result?;
        Ok((called, bodies))
    }

    pub(crate) fn check_selectseq_value(
        &self,
        st: &St,
        value: &dive_value::Value,
    ) -> crate::Result<()> {
        let Some(policy) = &st.selectseq else {
            return Ok(());
        };
        let mut remaining = policy
            .remaining
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let initial = *remaining;
        let mut guard = Guard {
            evaluator: self,
            limit: policy.limit,
            visits: 0,
            analysis: true,
            selectseq: true,

            current_sequence: None,
        };
        let result = guard.native_storage(None, &St::constant(), Some(value));
        *remaining = initial.saturating_sub(guard.visits);
        if guard.visits > initial {
            return Err(EvalError::resource_limit("SelectSeq consumed value work"));
        }
        result
    }

    /// Starts one pure predicate replay with fresh invocation-local work.
    /// Cloning or shifting the returned state shares these counters across
    /// sequential sub-evaluations and nested calls. This establishes no purity,
    /// admission or truth; callers perform their own context/level checks.
    ///
    /// ```
    /// # fn replay(ev: &dive_eval::Evaluator, scope: &dive_eval::Scope,
    /// # expr: dive_ir::ExprId, st: &dive_eval::St) -> dive_eval::Result<dive_value::Value> {
    /// let invocation = ev.begin_pure_predicate(scope, st, 4096)?;
    /// ev.eval_scoped_pure(scope, expr, &invocation, 4096)
    /// # }
    /// ```
    ///
    /// # Errors
    /// A scope or existing invocation context belonging to another evaluator.
    pub fn begin_pure_predicate(&self, scope: &Scope, st: &St, limit: usize) -> crate::Result<St> {
        self.validate_scope(scope)?;
        if st.selectseq.as_ref().is_some_and(|policy| {
            !std::sync::Weak::ptr_eq(&policy.context, &std::sync::Arc::downgrade(&self.core))
        }) {
            return Err(EvalError::unsupported(
                "foreign pure predicate invocation context",
            ));
        }
        let mut bounded = st.clone();
        bounded.selectseq = Some(std::sync::Arc::new(crate::env::SelectSeqBudget {
            limit,
            wrapper: scope.wrapper,
            context: std::sync::Arc::downgrade(&self.core),
            remaining: std::sync::Mutex::new(limit.saturating_mul(64).min(50_000)),
        }));
        bounded.selectseq_callback = false;
        Ok(bounded)
    }

    /// Evaluates a previously checked pure scoped predicate with invocation-local
    /// bounded `SelectSeq` callbacks. Ordinary scoped evaluation is unchanged.
    /// Every concrete element consumes shared work before its original callback
    /// runs. An existing invocation context shares its remaining work and
    /// original limit; otherwise this starts a fresh predicate invocation.
    ///
    /// # Errors
    /// Any evaluation, ownership, callback limitation or work-limit error.
    pub fn eval_scoped_pure(
        &self,
        scope: &Scope,
        expr: ExprId,
        st: &St,
        limit: usize,
    ) -> crate::Result<dive_value::Value> {
        self.validate_scope(scope)?;
        if let Some(policy) = &st.selectseq {
            if !std::sync::Weak::ptr_eq(&policy.context, &std::sync::Arc::downgrade(&self.core)) {
                return Err(EvalError::unsupported(
                    "foreign pure predicate invocation context",
                ));
            }
            self.eval_scoped(scope, expr, st)
        } else {
            let bounded = self.begin_pure_predicate(scope, st, limit)?;
            self.eval_scoped(scope, expr, &bounded)
        }
    }
}

struct Guard<'a> {
    evaluator: &'a Evaluator,
    limit: usize,
    visits: usize,
    analysis: bool,
    selectseq: bool,
    current_sequence: Option<dive_value::Fcn>,
}
impl Guard<'_> {
    fn shifted(st: &St) -> St {
        let mut shifted = match &st.s1 {
            StateRef::Vals(after) => St::state(after.clone()),
            StateRef::Empty if matches!(st.s0, StateRef::Empty) => St::constant(),
            // An absent successor may still be harmless (e.g. UNCHANGED x
            // while checking ENABLED). Any domain that actually reads it must
            // fail evaluation; no placeholder value is supplied.
            _ => St::empty(),
        };
        shifted.selectseq.clone_from(&st.selectseq);
        shifted.selectseq_callback = st.selectseq_callback;
        shifted
    }

    // Direct configured origin is still available here; captured operator
    // resolution deliberately does not acquire this route. Projection and
    // unused-actual inspection do not evaluate values or binder domains.
    fn configured_call(
        &mut self,
        scope: &Scope,
        constant: ConstId,
        args: &[Arg],
        st: &St,
        depth: usize,
    ) -> crate::Result<(dive_ir::DefId, Scope)> {
        let ev = self.evaluator;
        let Some(ConstBinding::Def(def)) = ev.constant_binding(constant) else {
            return Err(EvalError::unsupported(
                "unbound/value-bound finite operator",
            ));
        };
        let def = ev.redirected(def);
        let target = ev.admit_def(def)?;
        if target.scope != DefScope::TopLevel
            || target.recursive
            || target.std.is_some()
            || ev.is_value_override(def)
            || target.level > Level::Action
            || target.params.iter().any(|p| p.arity != 0)
        {
            return Err(EvalError::unsupported("configured finite operator target"));
        }
        if args.len() > self.limit {
            return Err(EvalError::resource_limit(
                "configured finite operator arguments",
            ));
        }
        let called = ev.scope_apply(scope, def, args)?;
        let saved = self.analysis;
        self.analysis = true;
        let result = (|| {
            for arg in args {
                let Arg::Expr { expr, .. } = arg else {
                    return Err(EvalError::unsupported(
                        "higher-order configured finite argument",
                    ));
                };
                self.walk(scope, *expr, st, depth + 1)?;
            }
            Ok((def, called))
        })();
        self.analysis = saved;
        result
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
        let (def, analysis) = if self.selectseq {
            let value = self.evaluator.core.op_value(&scope.env, operator)?;
            let resolved = self
                .evaluator
                .selectseq_value_analysis(scope, value, &mut remaining);
            self.visits = ceiling - remaining;
            let (def, analysis) = resolved?.ok_or_else(|| {
                EvalError::unsupported("SelectSeq higher-order/native callback context")
            })?;
            (Some(def), analysis)
        } else {
            let native =
                self.evaluator
                    .scope_native_operator_analysis(scope, operator, &mut remaining);
            self.visits = ceiling - remaining;
            if native?.is_some() {
                (None, scope.clone())
            } else {
                let (def, analysis) = self
                    .evaluator
                    .scope_operator_analysis(scope, operator)?
                    .ok_or_else(|| {
                        EvalError::unsupported("opaque/recursive/higher-order finite operator")
                    })?;
                (Some(def), analysis)
            }
        };
        let saved = self.analysis;
        self.analysis = true;
        let result: crate::Result<()> = (|| {
            let ceiling = self.limit.saturating_mul(64).min(100_000);
            let mut remaining = ceiling.saturating_sub(self.visits);
            let captures =
                self.evaluator
                    .operator_captures(&analysis, &mut remaining, self.selectseq);
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
        self.native_storage(Some(scope), st, None)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "bounded callback-free stored representation traversal"
    )]
    fn native_storage(
        &mut self,
        scope: Option<&Scope>,
        st: &St,
        value: Option<&dive_value::Value>,
    ) -> crate::Result<()> {
        use crate::env::{Dyn, Ev, Frame, OpVal, Sv};
        use std::sync::Arc;
        enum Work {
            Frame(Arc<Frame>, usize),
            Dynamic(Dyn, usize),
            Slot(Sv, usize),
            Value(dive_value::Value, usize),
        }
        let cap = self.limit.saturating_mul(64).min(50_000);
        let mut work = Vec::new();
        if let Some(scope) = scope {
            work.push(Work::Frame(scope.env.frame.clone(), 0));
            work.push(Work::Dynamic(scope.env.dynamic.clone(), 0));
        }
        if let Some(value) = value {
            work.push(Work::Value(value.clone(), 0));
        }
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
                    if self.selectseq {
                        continue;
                    }
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
                    if self.selectseq && dynamic.is_some() {
                        return Err(EvalError::unsupported(
                            "SelectSeq retained dynamic callback",
                        ));
                    }
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
                        // During capture storage inspection only, the current
                        // native input is checked one element at a time by the
                        // loop. Pointer identity admits no other stored value.
                        // Callback reads use a separate full consumed-value walk.
                        if self
                            .current_sequence
                            .as_ref()
                            .is_some_and(|sequence| sequence.ptr_eq(&function))
                        {
                            continue;
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
        if self.selectseq
            && matches!(
                node.expr,
                Expr::Quant { .. }
                    | Expr::SetMap { .. }
                    | Expr::SetFilter { .. }
                    | Expr::FcnCons { .. }
                    | Expr::RecFcn { .. }
                    | Expr::RecordSet(_)
                    | Expr::FcnSet { .. }
                    | Expr::Cartesian(_)
                    | Expr::Builtin {
                        op: BuiltinOp::PowerSet
                            | BuiltinOp::Times
                            | BuiltinOp::String
                            | BuiltinOp::Prime
                            | BuiltinOp::Unchanged
                            | BuiltinOp::Cdot
                            | BuiltinOp::Enabled
                            | BuiltinOp::Always
                            | BuiltinOp::Eventually
                            | BuiltinOp::LeadsTo
                            | BuiltinOp::WhilePlus,
                        ..
                    }
                    | Expr::Apply {
                        callee: OpRef::Builtin(
                            BuiltinOp::PowerSet
                                | BuiltinOp::Times
                                | BuiltinOp::String
                                | BuiltinOp::Prime
                                | BuiltinOp::Unchanged
                                | BuiltinOp::Cdot
                                | BuiltinOp::Enabled
                                | BuiltinOp::Always
                                | BuiltinOp::Eventually
                                | BuiltinOp::LeadsTo
                                | BuiltinOp::WhilePlus
                        ),
                        ..
                    }
                    | Expr::Enabled(_)
                    | Expr::Always(_)
                    | Expr::Eventually(_)
                    | Expr::LeadsTo(_, _)
                    | Expr::WhilePlus(_, _)
                    | Expr::Wf { .. }
                    | Expr::Sf { .. }
                    | Expr::TemporalQuant { .. }
                    | Expr::StepBox { .. }
                    | Expr::StepAngle { .. }
                    | Expr::Cdot(_, _)
                    | Expr::Prime(_)
                    | Expr::Unchanged(_)
                    | Expr::Var { primed: true, .. }
            )
        {
            return Err(EvalError::unsupported(
                "SelectSeq binder/provider/action callback",
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
            Expr::Subst { body, bindings } => {
                if self.analysis
                    && bindings.iter().any(|binding| {
                        matches!(
                            binding.value,
                            Arg::Expr {
                                pass: dive_ir::Pass::Strict,
                                ..
                            }
                        )
                    })
                {
                    return Err(EvalError::unsupported(
                        "eager substitution in finite analysis",
                    ));
                }
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
                    Some(ConstBinding::Value(value)) => {
                        if self.selectseq {
                            self.native_storage(None, &St::constant(), Some(&value))?;
                        }
                        Ok(())
                    }
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
            let standard = match op.clone() {
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
                OpRef::Const(constant) => {
                    let (def, called) =
                        self.configured_call(scope, constant, args, st, depth + 1)?;
                    return self.walk(&called, ev.ir().def(def).body, st, depth + 1);
                }
                OpRef::Subst(_) => {
                    return Err(EvalError::unsupported("dynamic finite binding operator"));
                }
                _ => None,
            };
            if self.selectseq
                && standard.is_some_and(|s| {
                    s.impure()
                        || s.creates_runtime_names()
                        || matches!(
                            s.support(),
                            dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                        )
                        || matches!(
                            s,
                            StdOp::SequencesSeq | StdOp::NaturalsNat | StdOp::IntegersInt
                        )
                })
            {
                return Err(EvalError::unsupported(
                    "SelectSeq effect/provider/native callback",
                ));
            }
            if self.selectseq && standard == Some(StdOp::SequencesSelectSeq) {
                return Err(EvalError::unsupported(
                    "SelectSeq nested higher-order callback",
                ));
            }
            if standard == Some(StdOp::SequencesSelectSeq) {
                let ceiling = self.limit.saturating_mul(64).min(100_000);
                let mut remaining = ceiling.saturating_sub(self.visits);
                // Inspect structure only. Concrete input and formal checks occur in the native loop.
                let result = ev.scope_selectseq_call(scope, &op, args, &mut remaining);
                self.visits = ceiling - remaining;
                let (sequence, _) =
                    result?.ok_or_else(|| EvalError::unsupported("SelectSeq callback shape"))?;
                return self.walk(scope, sequence, st, depth + 1);
            }
            if standard.is_some() && args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                return Err(EvalError::unsupported(
                    "higher-order finite standard argument",
                ));
            }
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

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "owned callback storage and cache assertions"
)]
mod selectseq_tests {
    use super::*;
    use crate::env::{Dyn, Ev, OpVal, Sv, Thunk};
    use dive_value::{Ctx, DeferredKind, DeferredSet, Interner, Value};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn evaluator() -> Evaluator {
        evaluator_with_options(crate::Options::tlc())
    }

    fn evaluator_with_options(options: crate::Options) -> Evaluator {
        let source = "---- MODULE SelectStorage ----\nEXTENDS Sequences, TLC\nVARIABLE S\nGood=={0}\nBox(y)==[TRUE]_<<>>\nAngle(y)==<<TRUE>>_<<>>\nBoxRun==SelectSeq(<<0>>,Box)\nAngleRun==SelectSeq(<<0>>,Angle)\nEvil==PrintT(0)\nAlways(y)==TRUE\nMany==SelectSeq(<<0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0>>,Always)\nStateful(y)==y \\in S\nStateRun==SelectSeq(<<0>>,Stateful)\nHigherOrder(H(_),a)==H(a)\nHigher(y)==HigherOrder(Len,y)\nHigherRun==SelectSeq(<< <<0>> >>,Higher)\nSequence(s)==SelectSeq(s,Always)\nPriority(s)==SelectSeq(s,LAMBDA y:y[1]=0)\nTwin(s,t)==SelectSeq(s,LAMBDA y:y[1]=0)\nAliasRead(s)==SelectSeq(s,LAMBDA y:Len(s)=2)\nWrap(v)==v\nPrimed==Many'\nCached==Many\nAction==S'=S /\\ Cached=Many\nCompose==(S'=S) \\cdot (S'=S)\nEmptyCompose==FALSE \\cdot (S'=S)\nShortCompose==FALSE /\\ Compose\nWarmCompose==IF Cached=Many THEN Compose ELSE FALSE\nForward(H(_),s)==SelectSeq(s,H)\nOuter(v)==Forward(LAMBDA y: y \\in v,<<0,0>>)\nUnused(v)==SelectSeq(<<0>>, LAMBDA y: TRUE)\n====";
        let analysis =
            dive_sema::analyze_source("SelectStorage.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        Evaluator::new(crate::Model::new(ir), options).expect("evaluator")
    }

    #[derive(Debug, Default)]
    struct Provider(AtomicUsize);
    impl DeferredSet for Provider {
        fn kind(&self) -> DeferredKind {
            DeferredKind::Map
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![0.into()])
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Ok(true))
        }
    }

    #[test]
    fn bounded_callbacks_ignore_ordinary_and_poisoned_thunk_caches_without_mutating_them() {
        let ev = evaluator();
        let outer = ev.ir().root_def("Outer").expect("outer");
        let good = ev.ir().root_def("Good").expect("good");
        let source = ev.scope(good).expect("source");
        let provider = Arc::new(Provider::default());
        let foreign = Value::string(&Interner::new(), "foreign cached name");
        let stored = Value::set(vec![0.into()], Ctx::tlc()).expect("stored set");
        for cached in [
            None,
            Some(stored),
            Some(foreign),
            Some(Value::deferred(provider.clone(), Ctx::tlc())),
        ] {
            let thunk = Thunk::new(ev.ir().def(good).body, source.env.clone(), true);
            if let Some(value) = &cached {
                thunk.store(&St::constant(), &Ev::V(value.clone()));
            }
            let mut scope = ev.scope(outer).expect("outer scope");
            scope.env.bind(0, Sv::Thunk(thunk.clone()));
            let result =
                ev.eval_scoped_pure(&scope, ev.ir().def(outer).body, &St::constant(), 4096);
            assert_eq!(
                result.expect("safe source independently recomputed"),
                Value::tuple(vec![0.into(), 0.into()])
            );
            let after = thunk.cached(&St::constant());
            match (cached.as_ref(), after) {
                (None, None) => {}
                (Some(Value::Set(before)), Some(Ev::V(Value::Set(after)))) => {
                    assert!(before.ptr_eq(&after));
                }
                (Some(Value::Str(before)), Some(Ev::V(Value::Str(after)))) => {
                    assert_eq!(before, &after);
                }
                _ => panic!("caller cache changed"),
            }
        }
        assert_eq!(provider.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn unused_stored_captures_retained_dynamic_and_foreign_scope_stay_refused() {
        let ev = evaluator();
        let unused = ev.ir().root_def("Unused").expect("unused");
        let always = ev.ir().root_def("Always").expect("always");
        let provider = Arc::new(Provider::default());
        let foreign = Value::string(&Interner::new(), "unused foreign");
        for value in [foreign, Value::deferred(provider.clone(), Ctx::tlc())] {
            let mut scope = ev.scope(unused).expect("scope");
            scope.env.bind(0, Sv::Val(value));
            assert!(
                ev.eval_scoped_pure(&scope, ev.ir().def(unused).body, &St::constant(), 4096)
                    .is_err()
            );
        }
        assert_eq!(provider.0.load(Ordering::Relaxed), 0);
        let scope = ev.scope(always).expect("scope");
        let op = OpVal::Def {
            def: always,
            parent: None,
            dynamic: Some(Dyn::default()),
        };
        assert!(
            ev.selectseq_context(&scope, op, None, None, &mut 1000)
                .is_err()
        );
        let other = evaluator();
        assert!(
            ev.scope_selectseq_call(
                &other
                    .scope(other.ir().root_def("Many").expect("many"))
                    .expect("foreign scope"),
                &OpRef::Std(StdOp::SequencesSelectSeq),
                &[],
                &mut 1000
            )
            .is_err()
        );
    }

    #[test]
    fn duplicate_callbacks_charge_aggregate_work_and_new_invocations_get_fresh_budget() {
        let ev = evaluator();
        let many = ev.ir().root_def("Many").expect("many");
        let scope = ev.scope(many).expect("scope");
        assert!(
            ev.eval_scoped_pure(&scope, ev.ir().def(many).body, &St::constant(), 1)
                .expect_err("aggregate duplicate work")
                .to_string()
                .contains("work")
        );
        for _ in 0..2 {
            assert_eq!(
                ev.eval_scoped_pure(&scope, ev.ir().def(many).body, &St::constant(), 4096)
                    .expect("fresh invocation"),
                Value::tuple(vec![0.into(); 20])
            );
        }
    }

    #[test]
    fn reached_state_values_and_unused_effectful_sources_are_checked_without_provider_access() {
        let ev = evaluator();
        let run = ev.ir().root_def("StateRun").expect("run");
        let scope = ev.scope(run).expect("scope");
        let provider = Arc::new(Provider::default());
        for value in [
            Value::deferred(provider.clone(), Ctx::tlc()),
            Value::string(&Interner::new(), "foreign state"),
            Value::set(vec![0.into()], Ctx::dive()).expect("other profile set"),
        ] {
            let st = St::state(Arc::from([Some(value)]));
            assert_eq!(
                ev.eval_scoped_pure(&scope, ev.ir().def(run).body, &st, 4096)
                    .expect_err("reached state storage checked before membership")
                    .class(),
                crate::ErrorClass::Unsupported
            );
        }
        assert_eq!(provider.0.load(Ordering::Relaxed), 0);
        let evil = ev.ir().root_def("Evil").expect("evil");
        let unused = ev.ir().root_def("Unused").expect("unused");
        let mut captured = ev.scope(unused).expect("scope");
        captured.env.bind(
            0,
            Sv::Thunk(Thunk::new(
                ev.ir().def(evil).body,
                ev.scope(evil).expect("evil scope").env,
                true,
            )),
        );
        assert!(
            ev.eval_scoped_pure(&captured, ev.ir().def(unused).body, &St::constant(), 4096)
                .is_err()
        );
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }

    #[test]
    fn warm_constant_and_whole_selection_thunks_cannot_bypass_fresh_budgets() {
        let ev = evaluator();
        let cached = ev.ir().root_def("Cached").expect("cached");
        let primed = ev.ir().root_def("Primed").expect("primed");
        let wrap = ev.ir().root_def("Wrap").expect("wrap");
        let caller = ev.scope(cached).expect("caller");
        let captured = ev
            .scope_apply(
                &ev.scope(wrap).expect("wrapper"),
                wrap,
                &[Arg::Expr {
                    expr: ev.ir().def(cached).body,
                    pass: dive_ir::Pass::Lazy,
                }],
            )
            .expect("whole selection thunk");
        assert_eq!(
            ev.eval_scoped(&captured, ev.ir().def(wrap).body, &St::constant())
                .expect("warm raw caches"),
            Value::tuple(vec![0.into(); 20])
        );
        let Sv::Thunk(thunk) = &captured.env.frame.slots[0] else {
            panic!("thunk expected");
        };
        let Some(Ev::V(Value::Fcn(before))) = thunk.cached(&St::constant()) else {
            panic!("warmed tuple expected");
        };
        for (scope, expr) in [
            (&captured, ev.ir().def(wrap).body),
            (&caller, ev.ir().def(cached).body),
            (&ev.scope(primed).expect("prime"), ev.ir().def(primed).body),
        ] {
            for _ in 0..2 {
                assert!(
                    ev.eval_scoped_pure(scope, expr, &St::constant(), 1)
                        .expect_err("fresh bounded invocation")
                        .to_string()
                        .contains("work")
                );
                assert_eq!(
                    ev.eval_scoped_pure(scope, expr, &St::constant(), 4096)
                        .expect("independent success"),
                    Value::tuple(vec![0.into(); 20])
                );
            }
        }
        let Some(Ev::V(Value::Fcn(after))) = thunk.cached(&St::constant()) else {
            panic!("caller cache retained");
        };
        assert!(before.ptr_eq(&after));
        assert_eq!(
            ev.eval_scoped(&captured, ev.ir().def(wrap).body, &St::constant())
                .expect("raw cache unchanged"),
            Value::tuple(vec![0.into(); 20])
        );
    }

    #[test]
    fn stored_function_sequence_profile_order_is_checked_before_callbacks() {
        let ev = evaluator();
        let sequence = ev.ir().root_def("Sequence").expect("sequence");
        for (ctx, accepted) in [(Ctx::tlc(), true), (Ctx::dive(), false)] {
            let element = if accepted {
                Value::Int(0)
            } else {
                Value::set(vec![0.into()], ctx).expect("stored element in other profile")
            };
            // 1..n functions use tuple storage; its integer keys have no
            // profile-sensitive order. Check the actual stored elements.
            let input = Value::fcn(vec![(1.into(), element.clone()), (2.into(), element)], ctx)
                .expect("function sequence");
            let called = ev
                .scope_call_values(&ev.scope(sequence).expect("scope"), sequence, vec![input])
                .expect("concrete sequence");
            let result =
                ev.eval_scoped_pure(&called, ev.ir().def(sequence).body, &St::constant(), 4096);
            if accepted {
                assert_eq!(
                    result.expect("owned order"),
                    Value::tuple(vec![0.into(), 0.into()])
                );
            } else {
                assert!(
                    result
                        .expect_err("wrong sequence order")
                        .to_string()
                        .contains("order")
                );
            }
        }
    }

    #[test]
    fn stored_outer_sequence_alias_keeps_first_callback_error_before_later_resource_input() {
        for options in [crate::Options::tlc(), crate::Options::default()] {
            let ev = evaluator_with_options(options);
            let priority = ev.ir().root_def("Priority").expect("priority");
            let large = Value::tuple(vec![0.into(); 30_000]);
            let run = |sequence| {
                let called = ev
                    .scope_call_values(
                        &ev.scope(priority).expect("scope"),
                        priority,
                        vec![sequence],
                    )
                    .expect("stored sequence argument");
                let raw = ev.eval_scoped(&called, ev.ir().def(priority).body, &St::constant());
                let bounded =
                    ev.eval_scoped_pure(&called, ev.ir().def(priority).body, &St::constant(), 4096);
                (raw, bounded)
            };
            let (raw, bounded) = run(Value::tuple(vec![0.into(), large.clone()]));
            let raw = raw.expect_err("first scalar callback error");
            assert_eq!(raw.ec(), crate::Ec::JavaMethodOverride);
            assert_eq!(
                raw.to_string(),
                bounded.expect_err("same first callback error").to_string()
            );
            let (_, reverse) = run(Value::tuple(vec![large, 0.into()]));
            assert!(
                reverse
                    .expect_err("first reached resource input")
                    .to_string()
                    .contains("work")
            );
            let duplicates = Value::tuple(vec![Value::tuple(vec![0.into()]); 20]);
            let called = ev
                .scope_call_values(
                    &ev.scope(priority).expect("scope"),
                    priority,
                    vec![duplicates.clone()],
                )
                .expect("duplicate input");
            assert!(
                ev.eval_scoped_pure(&called, ev.ir().def(priority).body, &St::constant(), 1)
                    .expect_err("duplicates charge individually")
                    .to_string()
                    .contains("work")
            );
            assert_eq!(
                ev.eval_scoped_pure(&called, ev.ir().def(priority).body, &St::constant(), 4096)
                    .expect("fresh duplicate invocation"),
                duplicates
            );
        }
    }

    #[test]
    fn current_sequence_alias_exempts_no_separate_storage_and_reads_remain_fully_checked() {
        for options in [crate::Options::tlc(), crate::Options::default()] {
            let ev = evaluator_with_options(options);
            let large = Value::tuple(vec![0.into(); 30_000]);
            let first = Value::tuple(vec![0.into(), large.clone()]);
            let same_content = Value::tuple(vec![0.into(), large]);
            assert!(
                !first
                    .as_fcn()
                    .expect("tuple")
                    .ptr_eq(same_content.as_fcn().expect("tuple"))
            );
            let twin = ev.ir().root_def("Twin").expect("twin");
            let called = ev
                .scope_call_values(
                    &ev.scope(twin).expect("scope"),
                    twin,
                    vec![first, same_content],
                )
                .expect("distinct stored arguments");
            assert!(
                ev.eval_scoped_pure(&called, ev.ir().def(twin).body, &St::constant(), 4096)
                    .expect_err("unused separate storage receives no exemption")
                    .to_string()
                    .contains("work")
            );
            let alias = ev.ir().root_def("AliasRead").expect("alias read");
            let provider = Arc::new(Provider::default());
            for second in [
                Value::tuple(vec![0.into(); 30_000]),
                Value::deferred(provider.clone(), ev.options().ctx),
            ] {
                let called = ev
                    .scope_call_values(
                        &ev.scope(alias).expect("scope"),
                        alias,
                        vec![Value::tuple(vec![0.into(), second])],
                    )
                    .expect("alias input");
                // Tuple construction hashes its elements and may touch the
                // fixture provider. Evaluation itself must add no access.
                let before = provider.0.load(Ordering::Relaxed);
                assert!(
                    ev.eval_scoped_pure(&called, ev.ir().def(alias).body, &St::constant(), 4096)
                        .is_err(),
                    "actually reading the outer input performs full consumed-value checks"
                );
                assert_eq!(provider.0.load(Ordering::Relaxed), before);
            }
        }
    }

    #[test]
    fn configured_and_native_predicate_targets_and_incompatible_lexical_owners_refuse() {
        let analysis = dive_sema::analyze_source(
            "SelectTargets.tla",
            "---- MODULE SelectTargets ----\nEXTENDS Sequences\nCONSTANT F(_)\nKeep(y)==TRUE\nConfigured==SelectSeq(<<0>>,F)\nDirect==SelectSeq(<<0>>,Keep)\n====",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let keep = ir.root_def("Keep").expect("keep");
        let mut model = crate::Model::new(ir.clone());
        model.constants[0] = Some(ConstBinding::Def(keep));
        let ev = Evaluator::new(model, crate::Options::tlc()).expect("configured evaluator");
        for name in ["Configured", "Direct"] {
            let def = ir.root_def(name).expect("call");
            assert_eq!(
                ev.eval_scoped(
                    &ev.scope(def).expect("scope"),
                    ir.def(def).body,
                    &St::constant()
                )
                .expect("raw configured callback"),
                Value::tuple(vec![0.into()])
            );
            assert!(
                ev.eval_scoped_pure(
                    &ev.scope(def).expect("scope"),
                    ir.def(def).body,
                    &St::constant(),
                    4096
                )
                .is_err(),
                "{name}"
            );
        }
        let owned = evaluator();
        let unused = owned.ir().root_def("Unused").expect("unused");
        let always = owned.ir().root_def("Always").expect("always");
        let inline = owned
            .ir()
            .defs()
            .iter()
            .enumerate()
            .find_map(|(index, def)| {
                matches!(def.scope,dive_ir::DefScope::Inline{parent} if parent==unused)
                    .then_some(dive_ir::DefId(u32::try_from(index).expect("index")))
            })
            .expect("inline lambda");
        let mut scope = owned.scope(unused).expect("scope");
        scope.env.bind(0, Sv::Val(0.into()));
        let op = OpVal::Def {
            def: inline,
            parent: Some(owned.scope(always).expect("wrong owner").env.frame),
            dynamic: None,
        };
        assert!(
            owned
                .selectseq_context(&scope, op, None, None, &mut 1000)
                .expect_err("lexical owner")
                .to_string()
                .contains("owner")
        );
        for op in [
            OpVal::Std(StdOp::SequencesLen),
            OpVal::Std(StdOp::TlcRandomElement),
            OpVal::Std(StdOp::TlcTLCGet),
        ] {
            scope.env.bind(0, Sv::Op(op));
            assert!(
                owned
                    .eval_scoped_pure(&scope, owned.ir().def(unused).body, &St::constant(), 4096)
                    .is_err()
            );
        }
        let higher = owned
            .ir()
            .root_def("HigherRun")
            .expect("higher-order native body");
        assert!(
            owned
                .eval_scoped_pure(
                    &owned.scope(higher).expect("scope"),
                    owned.ir().def(higher).body,
                    &St::constant(),
                    4096
                )
                .is_err()
        );
        assert_eq!(owned.take_output(), Vec::<String>::new());
    }

    #[test]
    fn selectseq_explicit_action_syntax_refuses_even_when_raw_callback_is_state_invariant() {
        let ev = evaluator();
        for name in ["BoxRun", "AngleRun"] {
            let def = ev.ir().root_def(name).expect("action shape");
            let scope = ev.scope(def).expect("scope");
            ev.eval_scoped(&scope, ev.ir().def(def).body, &St::constant())
                .expect("original raw invariant action callback");
            let result = ev.eval_scoped_pure(&scope, ev.ir().def(def).body, &St::constant(), 4096);
            assert!(
                matches!(result,Err(ref error) if error.class()==crate::ErrorClass::Unsupported)
            );
        }
    }

    #[test]
    fn selectseq_direct_shapes_refuse_forged_levels_and_warm_thunks_without_admission_flags() {
        let source = "---- MODULE SelectDirectShapes ----\nEXTENDS Sequences\nVARIABLE S\nRECURSIVE Rec(_)\nRec(y)==Rec(y)\nBox(y)==[TRUE]_<<>>\nAngle(y)==<<TRUE>>_<<>>\nPrimeBool(y)==TRUE'\nUnchangedBool(y)==UNCHANGED <<>>\nEnabledBool(y)==ENABLED FALSE\nTemporal(y)==[] TRUE\nFair(y)==WF_S(S'=S)\nPrimedRead(y)==S'=S\nPlaceholder(y)==TRUE\nHigher(O(_), y)==O(y)\nHigh(y)==Higher(Len,<<0>>) = 1\nB==SelectSeq(<<0>>,Box)\nA==SelectSeq(<<0>>,Angle)\nP==SelectSeq(<<0>>,PrimeBool)\nU==SelectSeq(<<0>>,UnchangedBool)\nE==SelectSeq(<<0>>,EnabledBool)\nT==SelectSeq(<<0>>,Placeholder)\nF==SelectSeq(<<0>>,Placeholder)\nV==SelectSeq(<<0>>,Placeholder)\nR==SelectSeq(<<0>>,Rec)\nH==SelectSeq(<<0>>,High)\nWrap(v)==v\n====";
        let analysis = dive_sema::analyze_source(
            "SelectDirectShapes.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = dive_ir::lower(&analysis).expect("IR");
        for builtin in [false, true] {
            let mut json = serde_json::to_value(&ir).expect("serialize");
            // Public serialized IR metadata is no per-call action/temporal
            // admission. Keep actual syntax, scopes, recursion and arities.
            for nodes in ["exprs", "defs"] {
                for node in json[nodes].as_array_mut().expect("array") {
                    node["level"] = serde_json::to_value(dive_ir::Level::Constant).expect("level");
                    node["effects"] = serde_json::to_value(Effects::NONE).expect("effects");
                }
            }
            // The source frontend correctly rejects temporal/action callback
            // actuals. Direct public IR mechanics use valid placeholder calls,
            // then supply the otherwise inadmissible owned source definitions.
            for (run, target) in [("T", "Temporal"), ("F", "Fair"), ("V", "PrimedRead")] {
                let body = ir.def(ir.root_def(run).expect("run")).body;
                let Expr::Apply { callee, args } = ir.expr(body) else {
                    panic!("select application")
                };
                let mut args = args.to_vec();
                let Arg::Op { op, .. } = &mut args[1] else {
                    panic!("callback argument")
                };
                *op = OpRef::Def(ir.root_def(target).expect("target"));
                json["exprs"][body.index()]["expr"] = serde_json::to_value(Expr::Apply {
                    callee: callee.clone(),
                    args: args.into(),
                })
                .expect("owned application");
            }
            if builtin {
                for (name, op) in [
                    ("PrimeBool", BuiltinOp::Prime),
                    ("UnchangedBool", BuiltinOp::Unchanged),
                ] {
                    let body = ir.def(ir.root_def(name).expect("body")).body;
                    let child = match ir.expr(body) {
                        Expr::Prime(child) | Expr::Unchanged(child) => *child,
                        _ => panic!("original syntax"),
                    };
                    json["exprs"][body.index()]["expr"] = serde_json::to_value(Expr::Builtin {
                        op,
                        args: vec![child].into(),
                    })
                    .expect("builtin syntax");
                }
            }
            let fake: dive_ir::Ir = serde_json::from_value(json).expect("owned IR");
            let ev = Evaluator::new(crate::Model::new(Arc::new(fake)), crate::Options::tlc())
                .expect("evaluator");
            let wrap = ev.ir().root_def("Wrap").expect("wrap");
            for name in ["B", "A", "P", "U", "E", "T", "F", "V", "R", "H"] {
                let run = ev.ir().root_def(name).expect("run");
                let caller = ev.scope(run).expect("caller");
                let scope = ev
                    .scope_apply(
                        &caller,
                        wrap,
                        &[Arg::Expr {
                            expr: ev.ir().def(run).body,
                            pass: dive_ir::Pass::Lazy,
                        }],
                    )
                    .expect("lazy original route");
                let Sv::Thunk(thunk) = &scope.env.frame.slots[0] else {
                    panic!("thunk")
                };
                for warm in [false, true] {
                    if warm {
                        // Synthetic existing representation; the bounded mode
                        // must ignore it and reach the actual unsupported source.
                        thunk.store(&St::constant(), &Ev::V(Value::tuple(vec![0.into()])));
                    }
                    let result =
                        ev.eval_scoped_pure(&scope, ev.ir().def(wrap).body, &St::constant(), 4096);
                    assert!(
                        matches!(result,Err(ref error) if error.class()==crate::ErrorClass::Unsupported),
                        "{name}"
                    );
                }
            }
        }
    }

    #[test]
    fn selectseq_invalid_compound_input_refuses_nested_providers_before_error_rendering() {
        for options in [crate::Options::tlc(), crate::Options::default()] {
            let ev = evaluator_with_options(options);
            let sequence = ev.ir().root_def("Sequence").expect("sequence");
            let priority = ev.ir().root_def("Priority").expect("priority");
            let provider = Arc::new(Provider::default());
            let deferred = Value::deferred(provider.clone(), ev.options().ctx);
            let key = ev.core.names.intern("field");
            let invalid = [
                deferred.clone(),
                Value::set_raw(vec![deferred.clone()], ev.options().ctx),
                Value::record(vec![(key, deferred.clone())], ev.options().ctx)
                    .expect("record fixture"),
                Value::fcn(vec![(2.into(), deferred.clone())], ev.options().ctx)
                    .expect("function fixture"),
            ];
            for input in invalid {
                let scope = ev
                    .scope_call_values(&ev.scope(sequence).expect("scope"), sequence, vec![input])
                    .expect("input");
                let before = provider.0.load(Ordering::Relaxed);
                let result =
                    ev.eval_scoped_pure(&scope, ev.ir().def(sequence).body, &St::constant(), 4096);
                assert!(
                    matches!(result,Err(ref error) if error.class()==crate::ErrorClass::Unsupported)
                );
                assert_eq!(provider.0.load(Ordering::Relaxed), before);
            }
            for limit in [0, 4096] {
                let scope = ev
                    .scope_call_values(
                        &ev.scope(sequence).expect("scope"),
                        sequence,
                        vec![7.into()],
                    )
                    .expect("scalar input");
                let raw = ev
                    .eval_scoped(&scope, ev.ir().def(sequence).body, &St::constant())
                    .expect_err("ordinary invalid scalar first argument");
                let bounded = ev
                    .eval_scoped_pure(&scope, ev.ir().def(sequence).body, &St::constant(), limit)
                    .expect_err("same ordinary invalid scalar first argument");
                assert_eq!(raw.ec(), crate::Ec::ArgumentError);
                assert_eq!(
                    (raw.ec(), raw.message(), raw.origin()),
                    (bounded.ec(), bounded.message(), bounded.origin())
                );
            }
            let scope = ev
                .scope_call_values(
                    &ev.scope(priority).expect("scope"),
                    priority,
                    vec![Value::tuple(vec![0.into(), deferred.clone()])],
                )
                .expect("valid tuple fixture");
            let before = provider.0.load(Ordering::Relaxed);
            let result =
                ev.eval_scoped_pure(&scope, ev.ir().def(priority).body, &St::constant(), 4096);
            assert!(matches!(result,Err(ref error) if error.ec()==crate::Ec::JavaMethodOverride));
            assert_eq!(provider.0.load(Ordering::Relaxed), before);
            let scope = ev
                .scope_call_values(
                    &ev.scope(priority).expect("scope"),
                    priority,
                    vec![Value::tuple(vec![deferred, 0.into()])],
                )
                .expect("reversed valid tuple fixture");
            let before = provider.0.load(Ordering::Relaxed);
            let result =
                ev.eval_scoped_pure(&scope, ev.ir().def(priority).body, &St::constant(), 4096);
            assert!(
                matches!(result,Err(ref error) if error.class()==crate::ErrorClass::Unsupported)
            );
            assert_eq!(provider.0.load(Ordering::Relaxed), before);
        }
    }

    #[cfg(feature = "enumerate")]
    #[test]
    fn selectseq_enabled_search_preserves_shared_budget_cache_isolation_and_foreign_context_refusal()
     {
        let ev = evaluator();
        let action = ev.ir().root_def("Action").expect("action");
        let many = ev.ir().root_def("Many").expect("many");
        let scope = ev.scope(action).expect("scope");
        let st = St::state(Arc::from([Some(Value::Int(0))]));
        assert!(
            ev.enabled_scoped(
                &scope,
                ev.ir().def(action).body,
                None,
                Arc::from([Some(Value::Int(0))])
            )
            .expect("warm ordinary caches")
        );
        for _ in 0..2 {
            let error = ev
                .enabled_scoped_pure(&scope, ev.ir().def(action).body, None, &st, 1)
                .expect_err("warm whole constant selections still charge");
            assert_eq!(error.class(), crate::ErrorClass::ResourceLimit);
            assert!(
                ev.enabled_scoped_pure(&scope, ev.ir().def(action).body, None, &st, 4096)
                    .expect("fresh whole enabled invocation")
            );
        }
        let minimum = (1..4096)
            .find(|limit| {
                ev.eval_scoped_pure(&scope, ev.ir().def(many).body, &st, *limit)
                    .is_ok()
            })
            .expect("one subscript evaluation fits");
        assert_eq!(
            ev.enabled_scoped_pure(
                &scope,
                ev.ir().def(action).body,
                Some(ev.ir().def(many).body),
                &st,
                minimum
            )
            .expect_err("Changed before/after and action share one budget")
            .class(),
            crate::ErrorClass::ResourceLimit
        );
        let invocation = ev
            .begin_pure_predicate(&scope, &st, 4096)
            .expect("owned invocation");
        let other = evaluator();
        let target = other.ir().root_def("Action").expect("foreign action");
        assert_eq!(
            other
                .enabled_scoped_pure(
                    &other.scope(target).expect("foreign scope"),
                    other.ir().def(target).body,
                    None,
                    &invocation,
                    4096
                )
                .expect_err("foreign evaluator budget")
                .class(),
            crate::ErrorClass::Unsupported
        );
    }

    #[cfg(feature = "enumerate")]
    #[test]
    fn selectseq_bounded_enabled_composition_refuses_before_generator_but_keeps_shortcircuit() {
        let mut options = crate::Options::tlc();
        options.action_composition = crate::ActionComposition::Tla;
        let ev = evaluator_with_options(options);
        let st = St::state(Arc::from([Some(0.into())]));
        for name in ["Compose", "EmptyCompose", "WarmCompose"] {
            let def = ev.ir().root_def(name).expect("compose");
            let scope = ev.scope(def).expect("scope");
            ev.enabled_scoped(
                &scope,
                ev.ir().def(def).body,
                None,
                Arc::from([Some(0.into())]),
            )
            .expect("ordinary composition behavior including warm cache route");
            let error = ev
                .enabled_scoped_pure(&scope, ev.ir().def(def).body, None, &st, 4096)
                .expect_err("no bounded Gen delegation");
            assert!(
                error
                    .to_string()
                    .contains("bounded pure predicate ENABLED action composition")
            );
        }
        let def = ev.ir().root_def("ShortCompose").expect("short");
        assert!(
            !ev.enabled_scoped_pure(
                &ev.scope(def).expect("scope"),
                ev.ir().def(def).body,
                None,
                &st,
                4096
            )
            .expect("original false conjunction does not reach composition")
        );
    }

    #[test]
    fn invocation_context_ownership_and_state_control_transforms_preserve_resources() {
        let ev = evaluator();
        let many = ev.ir().root_def("Many").expect("many");
        let scope = ev.scope(many).expect("scope");
        let original = St::initial(Arc::from([Some(Value::Int(0))]));
        let invocation = ev
            .begin_pure_predicate(&scope, &original, 4096)
            .expect("invocation");
        let state = invocation.at_state(Arc::from([Some(Value::Int(1))]));
        let changed = state.with_control(crate::env::control::KEEP_LAZY);
        let shifted = Guard::shifted(&changed);
        for next in [
            &state,
            &changed,
            &shifted,
            &invocation.at_constant(),
            &invocation.at_action(Arc::from([Some(0.into())]), Arc::from([Some(1.into())])),
        ] {
            assert!(Arc::ptr_eq(
                invocation.selectseq.as_ref().expect("budget"),
                next.selectseq.as_ref().expect("preserved budget")
            ));
        }
        assert_eq!(state.control, crate::env::control::CLEAR);
        let other = evaluator();
        let target = other.ir().root_def("Many").expect("target");
        let foreign = other.scope(target).expect("other scope");
        assert!(
            other
                .begin_pure_predicate(&foreign, &invocation, 4096)
                .expect_err("foreign invocation initializer")
                .to_string()
                .contains("foreign")
        );
        let fresh = ev
            .begin_pure_predicate(&scope, &invocation, 4096)
            .expect("fresh same-evaluator invocation");
        assert!(!Arc::ptr_eq(
            invocation.selectseq.as_ref().expect("original budget"),
            fresh.selectseq.as_ref().expect("fresh budget")
        ));
        assert!(
            other
                .eval_scoped_pure(&foreign, other.ir().def(target).body, &invocation, 4096)
                .expect_err("foreign invocation")
                .to_string()
                .contains("foreign")
        );
        assert!(
            other
                .eval_scoped(&foreign, other.ir().def(target).body, &invocation)
                .expect_err("foreign explicitly bounded state")
                .to_string()
                .contains("foreign")
        );
    }
}
