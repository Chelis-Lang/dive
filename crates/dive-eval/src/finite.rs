//! Source projection of `SelectSeq` callbacks for the liveness planner and the
//! trace validator's temporal lowering. Nothing here evaluates a callback.

use crate::{ConstBinding, EvalError, Evaluator, Scope, SlotView, St};
use dive_ir::{Arg, BuiltinOp, ConstId, DefScope, Effects, Expr, ExprId, Level, OpRef, StdOp};

impl Evaluator {
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
        let bodies = self.selectseq_context(scope, value, work)?;
        Ok(Some((*sequence, bodies)))
    }

    fn selectseq_context(
        &self,
        scope: &Scope,
        value: crate::env::OpVal,
        work: &mut usize,
    ) -> crate::Result<Vec<(ExprId, Scope)>> {
        let (def, called) = self
            .selectseq_value_analysis(scope, value, work)?
            .ok_or_else(|| EvalError::unsupported("SelectSeq user callback target"))?;
        if self.ir().def(def).params.len() != 1 {
            return Err(EvalError::unsupported("SelectSeq unary value callback"));
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
            configured_depth: 0,
            query_allowed: true,
        };
        let result: crate::Result<()> = (|| {
            // Actual stored slots, including unused captures, are inspected.
            // No thunk result cache is read and no provider is normalized or
            // iterated here.
            guard.native_storage(Some(&called), None)?;
            for (body, captured) in &bodies {
                guard.walk(captured, *body, 0)?;
            }
            Ok(())
        })();
        *work = initial.saturating_sub(guard.visits);
        if guard.visits > initial {
            return Err(EvalError::resource_limit("SelectSeq callback work"));
        }
        result?;
        Ok(bodies)
    }
}

/// Walks `SelectSeq` callback sources at constant level, refusing effects,
/// binders, providers, priming and opaque operators. It reads no domain.
struct Guard<'a> {
    evaluator: &'a Evaluator,
    limit: usize,
    visits: usize,
    configured_depth: usize,
    query_allowed: bool,
}
impl Guard<'_> {
    // Direct configured origin is still available here; captured operator
    // resolution deliberately does not acquire this route. Projection and
    // unused-actual inspection do not evaluate values or binder domains.
    fn configured_call(
        &mut self,
        scope: &Scope,
        constant: ConstId,
        args: &[Arg],
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
        if ev
            .ir()
            .node(target.body)
            .effects
            .contains(Effects::USES_ENABLED)
        {
            return Err(EvalError::unsupported("configured ENABLED target"));
        }
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
        let saved = (self.configured_depth, self.query_allowed);
        self.configured_depth += 1;
        self.query_allowed = false;
        let result = (|| {
            for arg in args {
                let Arg::Expr { expr, .. } = arg else {
                    return Err(EvalError::unsupported(
                        "higher-order configured finite argument",
                    ));
                };
                self.walk(scope, *expr, depth + 1)?;
            }
            // Source-only configured inspection includes the target, not just its actuals.
            self.walk(&called, target.body, depth + 1)?;
            Ok((def, called))
        })();
        (self.configured_depth, self.query_allowed) = saved;
        result
    }

    fn operator_context(
        &mut self,
        scope: &Scope,
        operator: &OpRef,
        depth: usize,
    ) -> crate::Result<()> {
        let ceiling = self.limit.saturating_mul(64).min(100_000);
        let mut remaining = ceiling.saturating_sub(self.visits);
        let value = self.evaluator.core.op_value(&scope.env, operator)?;
        let resolved = self
            .evaluator
            .selectseq_value_analysis(scope, value, &mut remaining);
        self.visits = ceiling - remaining;
        let (def, analysis) = resolved?.ok_or_else(|| {
            EvalError::unsupported("SelectSeq higher-order/native callback context")
        })?;
        let saved = self.query_allowed;
        self.query_allowed = false;
        let result: crate::Result<()> = (|| {
            let ceiling = self.limit.saturating_mul(64).min(100_000);
            let mut remaining = ceiling.saturating_sub(self.visits);
            let captures = self
                .evaluator
                .operator_captures(&analysis, &mut remaining, true);
            self.visits = ceiling - remaining;
            let captures = captures?
                .ok_or_else(|| EvalError::unsupported("opaque finite operator context"))?;
            for (body, captured) in captures {
                self.walk(&captured, body, depth + 1)?;
            }
            self.walk(&analysis, self.evaluator.ir().def(def).body, depth + 1)
        })();
        self.query_allowed = saved;
        result
    }

    // Newly admitted native formals may read cached lexical values. Source
    // purity is insufficient to admit a deferred provider. Inspect storage,
    // names and order before native evaluation.
    #[allow(
        clippy::too_many_lines,
        reason = "bounded callback-free stored representation traversal"
    )]
    fn native_storage(
        &mut self,
        scope: Option<&Scope>,
        value: Option<&dive_value::Value>,
    ) -> crate::Result<()> {
        use crate::env::{Dyn, Frame, OpVal, Sv};
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
                    // The thunk's environment is inspected; its cached result
                    // is not read.
                    room(work.len(), 3)?;
                    work.push(Work::Frame(thunk.env.frame.clone(), depth + 1));
                    work.push(Work::Dynamic(thunk.env.dynamic.clone(), depth + 1));
                }
                Work::Slot(Sv::Op(op), _) => {
                    let OpVal::Def {
                        parent, dynamic, ..
                    } = *op
                    else {
                        continue;
                    };
                    if dynamic.is_some() {
                        return Err(EvalError::unsupported(
                            "SelectSeq retained dynamic callback",
                        ));
                    }
                    room(work.len(), usize::from(parent.is_some()))?;
                    if let Some(parent) = parent {
                        work.push(Work::Frame(parent, depth + 1));
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

    fn walk_data(&mut self, scope: &Scope, expr: ExprId, depth: usize) -> crate::Result<()> {
        let saved = self.query_allowed;
        self.query_allowed = false;
        let result = self.walk(scope, expr, depth);
        self.query_allowed = saved;
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit lexical binding and callback shape rules"
    )]
    fn walk(&mut self, scope: &Scope, expr: ExprId, depth: usize) -> crate::Result<()> {
        self.visits = self.visits.saturating_add(1);
        if depth > 256 || self.visits > self.limit.saturating_mul(64).min(100_000) {
            return Err(EvalError::resource_limit("finite binding traversal"));
        }
        let ev = self.evaluator;
        ev.admit_expr(expr)?;
        let node = ev.ir().node(expr);
        if node.effects.intersects(Effects::IMPURE | Effects::TEMPORAL)
            || ev.scope_shares_side_effects(scope)
        {
            return Err(EvalError::unsupported(
                "effects in finite binding preflight",
            ));
        }
        if node.effects.contains(Effects::USES_ENABLED)
            && (!self.query_allowed
                || !matches!(
                    node.expr,
                    Expr::StepBox { .. }
                        | Expr::StepAngle { .. }
                        | Expr::Enabled(_)
                        | Expr::Not(_)
                        | Expr::Junction { .. }
                        | Expr::Def(_)
                        | Expr::Apply {
                            callee: OpRef::Def(_),
                            ..
                        }
                        | Expr::Let { .. }
                        | Expr::Local(_)
                        | Expr::Subst { .. }
                        | Expr::SubstRef { .. }
                        | Expr::Quant {
                            kind: dive_ir::QuantKind::Forall | dive_ir::QuantKind::Exists,
                            ..
                        }
                        | Expr::Label { .. }
                        | Expr::Selected(_)
                ))
        {
            return Err(EvalError::unsupported(
                "ENABLED outside the Boolean spine or under priming",
            ));
        }
        if matches!(
            node.expr,
            Expr::Prime(_)
                | Expr::Var { primed: true, .. }
                | Expr::SubstRef { primed: true, .. }
                | Expr::Builtin {
                    op: BuiltinOp::Prime,
                    ..
                }
                | Expr::Apply {
                    callee: OpRef::Builtin(BuiltinOp::Prime),
                    ..
                }
        ) {
            return Err(EvalError::unsupported(
                "SelectSeq primed callback outside a query",
            ));
        }
        if matches!(
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
                | Expr::Unchanged(_)
        ) {
            return Err(EvalError::unsupported(
                "SelectSeq binder/provider/action callback",
            ));
        }
        match &node.expr {
            Expr::Not(body) | Expr::Selected(body) | Expr::Label { body, .. } => {
                return self.walk(scope, *body, depth + 1);
            }
            Expr::Junction { items, .. } => {
                for item in items {
                    self.walk(scope, *item, depth + 1)?;
                }
                return Ok(());
            }
            Expr::Let { body, .. } => {
                return self.walk(&ev.scope_let(scope, expr)?, *body, depth + 1);
            }
            Expr::Subst { body, bindings } => {
                if bindings.iter().any(|binding| {
                    matches!(
                        binding.value,
                        Arg::Expr {
                            pass: dive_ir::Pass::Strict,
                            ..
                        }
                    )
                }) {
                    return Err(EvalError::unsupported(
                        "eager substitution in finite analysis",
                    ));
                }
                return self.walk(&ev.scope_subst(scope, expr)?, *body, depth + 1);
            }
            Expr::Local(local) => {
                return match ev.scope_slot(scope, *local) {
                    SlotView::Value(_) | SlotView::AnalysisBound => Ok(()),
                    SlotView::Function(_) if self.configured_depth != 0 => Err(
                        EvalError::unsupported("recursive function in configured finite context"),
                    ),
                    SlotView::Lazy(body, sc) => self.walk(&sc, body, depth + 1),
                    _ => Err(EvalError::unsupported("opaque finite binding context")),
                };
            }
            Expr::SubstRef { decl, .. } => {
                return match ev.scope_substitution(scope, *decl, &St::constant(), false)? {
                    Some(SlotView::Value(value)) => self.native_storage(None, Some(&value)),
                    Some(SlotView::Lazy(body, sc)) => self.walk(&sc, body, depth + 1),
                    Some(SlotView::AnalysisBound) => Ok(()),
                    None if ev.ir().decl(*decl).variable => Ok(()),
                    None => match ev
                        .ir()
                        .decl(*decl)
                        .constant
                        .and_then(|c| ev.constant_binding(c))
                    {
                        Some(ConstBinding::Value(value)) => self.native_storage(None, Some(&value)),
                        Some(ConstBinding::Def(def)) => {
                            let def = ev.redirected(def);
                            self.walk(
                                &ev.scope_enter(scope, def)?,
                                ev.ir().def(def).body,
                                depth + 1,
                            )
                        }
                        None => Err(EvalError::unsupported(
                            "unbound substituted finite declaration",
                        )),
                    },
                    _ => Err(EvalError::unsupported("opaque substituted finite binding")),
                };
            }
            Expr::Const(c) => {
                return match ev.constant_binding(*c) {
                    Some(ConstBinding::Value(value)) => self.native_storage(None, Some(&value)),
                    Some(ConstBinding::Def(d)) => {
                        let d = ev.redirected(d);
                        self.walk_data(&ev.scope_enter(scope, d)?, ev.ir().def(d).body, depth + 1)
                    }
                    None => Err(EvalError::unsupported("unbound finite binding constant")),
                };
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
                            if self.configured_depth != 0 {
                                let Arg::Expr { expr, .. } = arg else {
                                    return Err(EvalError::unsupported(
                                        "higher-order configured finite argument",
                                    ));
                                };
                                // Unused helper actuals retain configured origin too.
                                // Inspect their caller scope before entering the body.
                                self.walk(scope, *expr, depth + 1)?;
                            } else if let Arg::Op { op, .. } = arg {
                                self.operator_context(scope, op, depth + 1)?;
                            }
                        }
                        return self.walk(&ev.scope_apply(scope, d, args)?, def.body, depth + 1);
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
                    self.operator_context(scope, &OpRef::Param(local), depth + 1)?;
                    for arg in args {
                        let Arg::Expr { expr, .. } = arg else {
                            return Err(EvalError::unsupported(
                                "higher-order finite binding actual",
                            ));
                        };
                        self.walk_data(scope, *expr, depth + 1)?;
                    }
                    if native.is_some() {
                        return Ok(());
                    }
                    let (def, called) =
                        ev.scope_apply_param(scope, local, args).ok_or_else(|| {
                            EvalError::unsupported("opaque finite binding operator")
                        })??;
                    return self.walk_data(&called, ev.ir().def(def).body, depth + 1);
                }
                OpRef::Const(constant) => {
                    let (def, called) = self.configured_call(scope, constant, args, depth + 1)?;
                    return self.walk_data(&called, ev.ir().def(def).body, depth + 1);
                }
                OpRef::Subst(_) => {
                    return Err(EvalError::unsupported("dynamic finite binding operator"));
                }
                _ => None,
            };
            if standard.is_some_and(|s| {
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
            }) {
                return Err(EvalError::unsupported(
                    "SelectSeq effect/provider/native callback",
                ));
            }
            if standard == Some(StdOp::SequencesSelectSeq) {
                return Err(EvalError::unsupported(
                    "SelectSeq nested higher-order callback",
                ));
            }
            if standard.is_some() && args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                return Err(EvalError::unsupported(
                    "higher-order finite standard argument",
                ));
            }
            if standard == Some(StdOp::TlcTLCGet) {
                return Err(EvalError::unsupported(
                    "effectful/checker finite binding operator",
                ));
            }
        }
        for child in node.expr.children() {
            self.walk_data(scope, child, depth + 1)?;
        }
        Ok(())
    }
}
