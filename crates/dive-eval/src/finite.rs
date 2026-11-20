//! Source projection of `SelectSeq` callbacks for the liveness planner and the
//! trace validator's temporal lowering. Nothing here evaluates a callback.

use crate::{ConstBinding, EvalError, Evaluator, Scope, SlotView, St};
use dive_ir::{Arg, BuiltinOp, ConstId, DefScope, Effects, Expr, ExprId, Level, OpRef, StdOp};

impl Evaluator {
    /// Inspects an authoritative `SelectSeq` call without evaluating its sequence
    /// or predicate. Returns the original sequence and callback/capture bodies
    /// in their lexical scopes. Pure supported unary user callbacks qualify.
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
        self.unary_predicate_call(scope, operator, args, work, false)
    }

    /// Projects the collection and unary callback bodies of `SelectSeq` or
    /// `Quantify` for temporal predicate analysis. The caller checks the
    /// returned bodies in their captured lexical scopes.
    ///
    /// # Errors
    /// Returns an error if the binding is opaque or analysis exhausts its budget.
    #[allow(
        clippy::type_complexity,
        reason = "collection and independently scoped callback bodies"
    )]
    pub fn scope_collection_predicate(
        &self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        work: &mut usize,
    ) -> crate::Result<Option<(ExprId, Vec<(ExprId, Scope)>)>> {
        self.unary_predicate_call(scope, operator, args, work, true)
    }

    #[allow(
        clippy::type_complexity,
        reason = "same scoped source projection as public wrappers"
    )]
    fn unary_predicate_call(
        &self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        work: &mut usize,
        quantify: bool,
    ) -> crate::Result<Option<(ExprId, Vec<(ExprId, Scope)>)>> {
        let Some((sequence, bodies)) =
            self.selectseq_projection(scope, operator, args, work, quantify)?
        else {
            return Ok(None);
        };
        let initial = *work;
        let mut guard = Guard {
            evaluator: self,
            limit: initial.div_ceil(64),
            visits: 0,
            configured_depth: 0,
            query_allowed: true,
        };
        let result = guard.callback_sources(&bodies, 0);
        *work = initial.saturating_sub(guard.visits);
        if guard.visits > initial {
            return Err(EvalError::resource_limit("SelectSeq callback work"));
        }
        result?;
        Ok(Some((sequence, bodies)))
    }

    // Metadata only. Nested selection is inspected by the caller's same Guard,
    // so neither a new callback nor a captured selector resets its work budget.
    #[allow(
        clippy::type_complexity,
        reason = "same source projection as the public wrapper"
    )]
    fn selectseq_projection(
        &self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        work: &mut usize,
        quantify: bool,
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
        let Some(standard) = standard.filter(|s| {
            *s == StdOp::SequencesSelectSeq || (quantify && *s == StdOp::FiniteSetsExtQuantify)
        }) else {
            return Ok(None);
        };
        if self.core.std_redirects.contains_key(&standard) {
            return Err(EvalError::unsupported(
                "SelectSeq configured standard target",
            ));
        }
        let [Arg::Expr { expr: sequence, .. }, Arg::Op { op, .. }] = args else {
            return Err(EvalError::unsupported(
                "SelectSeq sequence/operator argument shape",
            ));
        };
        self.admit_scoped_expr(scope, *sequence)?;
        // Validate the reference before looking up the value. The existing
        // resolver performs all definition/lexical/configured-origin checks.
        *work = work
            .checked_sub(self.core.consts.len().saturating_add(1))
            .ok_or_else(|| EvalError::resource_limit("SelectSeq reference work"))?;
        let Some(_) = self.scope_operator_analysis(scope, op)? else {
            return Err(EvalError::unsupported("SelectSeq user callback target"));
        };
        let value = self.core.op_value(&scope.env, op)?;
        let bodies = self.selectseq_sources(scope, value, work)?;
        Ok(Some((*sequence, bodies)))
    }

    fn selectseq_sources(
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
        bodies.push((self.ir().def(def).body, called));
        Ok(bodies)
    }
}

/// Walks `SelectSeq` callback sources without evaluating values or domains.
/// Effects, priming and opaque runtime providers remain explicit refusals.
struct Guard<'a> {
    evaluator: &'a Evaluator,
    limit: usize,
    visits: usize,
    configured_depth: usize,
    query_allowed: bool,
}

/// A successful-result kind, independent of effects, validity or finiteness.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultKind {
    NonString,
    Unknown,
}

impl Guard<'_> {
    fn kind_step(&mut self, depth: usize, work: usize) -> crate::Result<()> {
        self.visits = self.visits.saturating_add(work);
        if depth > 256 || self.visits > self.limit.saturating_mul(64).min(100_000) {
            return Err(EvalError::resource_limit("finite binding traversal"));
        }
        Ok(())
    }

    // Only the actual unmodified native target can use Concat's result-kind
    // rule. Operator parameters/configured/native-value admission stays separate.
    fn concat_without_names(
        &mut self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        depth: usize,
    ) -> crate::Result<bool> {
        self.kind_step(depth, 1)?;
        let ev = self.evaluator;
        ev.validate_scope(scope)?;
        if ev.core.std_redirects.contains_key(&StdOp::SequencesConcat) {
            return Ok(false);
        }
        // A native reference does not carry a configured-constant origin.
        // Keep the existing conservative authority boundary without extending
        // the separate operator-actual whitelist or inspecting stored values.
        for binding in &ev.core.consts {
            self.kind_step(depth, 1)?;
            if let Some(ConstBinding::Def(def)) = binding
                && ev.admit_def(ev.redirected(*def))?.std == Some(StdOp::SequencesConcat)
            {
                return Ok(false);
            }
        }
        for def in ev.core.def_values.keys() {
            self.kind_step(depth, 1)?;
            if ev.admit_def(*def)?.std == Some(StdOp::SequencesConcat) {
                return Ok(false);
            }
        }
        let [Arg::Expr { expr: left, .. }, Arg::Expr { expr: right, .. }] = args else {
            return Ok(false);
        };
        match operator {
            OpRef::Std(StdOp::SequencesConcat) => {}
            OpRef::Def(source) => {
                let target = ev.admit_def(*source)?;
                if ev.redirected(*source) != *source
                    || ev.is_value_override(*source)
                    || target.std != Some(StdOp::SequencesConcat)
                    || target.scope != DefScope::TopLevel
                    || target.recursive
                    || target.params.len() != 2
                    || target.params.iter().any(|parameter| parameter.arity != 0)
                {
                    return Ok(false);
                }
                ev.admit_args(scope, *source, args)?;
            }
            _ => return Ok(false),
        }
        ev.admit_scoped_expr(scope, *left)?;
        ev.admit_scoped_expr(scope, *right)?;
        let mut active = Vec::new();
        if self.result_kind(scope, *left, depth + 1, &mut active)? == ResultKind::NonString {
            return Ok(true);
        }
        Ok(self.result_kind(scope, *right, depth + 1, &mut active)? == ResultKind::NonString)
    }

    // Every retained owner remains live on the active path. Comparing only
    // context identities never hashes a Value or consults a thunk result cache.
    fn result_kind(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        self.kind_step(depth, active.len().saturating_add(1))?;
        self.evaluator.admit_scoped_expr(scope, expr)?;
        if scope.env.dynamic.at.is_some()
            || active.iter().any(|(prior, captured)| {
                *prior == expr
                    && std::sync::Arc::ptr_eq(&captured.env.frame, &scope.env.frame)
                    && captured.env.dynamic.cut == scope.env.dynamic.cut
                    && match (&captured.env.dynamic.subst, &scope.env.dynamic.subst) {
                        (None, None) => true,
                        (Some(left), Some(right)) => std::sync::Arc::ptr_eq(left, right),
                        _ => false,
                    }
            })
        {
            return Ok(ResultKind::Unknown);
        }
        let saved = active.len();
        active.push((expr, scope.clone()));
        let result = self.result_kind_inner(scope, expr, depth, active);
        active.truncate(saved);
        result.map_err(|error| error.at(expr))
    }

    // Constructor/result families are type facts even when their descendants
    // would fail or have effects. The independent full Guard walk MUST remain.
    fn result_kind_inner(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        let ev = self.evaluator;
        match &ev.ir().node(expr).expr {
            Expr::Bool(_)
            | Expr::Num(_)
            | Expr::Not(_)
            | Expr::Junction { .. }
            | Expr::Implies(_, _)
            | Expr::Equiv(_, _)
            | Expr::SetEnum(_)
            | Expr::Tuple(_)
            | Expr::Record(_)
            | Expr::RecordSet(_)
            | Expr::FcnSet { .. }
            | Expr::FcnCons { .. }
            | Expr::RecFcn { .. }
            | Expr::SetMap { .. }
            | Expr::SetFilter { .. }
            | Expr::Cartesian(_)
            | Expr::Quant {
                kind: dive_ir::QuantKind::Forall | dive_ir::QuantKind::Exists,
                ..
            } => Ok(ResultKind::NonString),
            Expr::Builtin { op, .. } if non_string_builtin(*op) => Ok(ResultKind::NonString),
            Expr::If { then, other, .. } => {
                let left = self.result_kind(scope, *then, depth + 1, active)?;
                let right = self.result_kind(scope, *other, depth + 1, active)?;
                Ok(join_kind(left, right))
            }
            Expr::Case { arms, other } => {
                let mut result = ResultKind::NonString;
                if arms.is_empty() {
                    return Ok(ResultKind::Unknown);
                }
                for arm in arms {
                    result = join_kind(
                        result,
                        self.result_kind(scope, arm.value, depth + 1, active)?,
                    );
                }
                if let Some(other) = other {
                    result = join_kind(result, self.result_kind(scope, *other, depth + 1, active)?);
                }
                Ok(result)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.result_kind(scope, *body, depth + 1, active)
            }
            _ => self.result_context_kind(scope, expr, depth, active),
        }
    }

    fn result_context_kind(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        let ev = self.evaluator;
        match &ev.ir().node(expr).expr {
            Expr::Let { body, defs } => {
                self.kind_step(
                    depth,
                    defs.len().saturating_add(scope.env.frame.slots.len()),
                )?;
                self.result_kind(&ev.scope_let(scope, expr)?, *body, depth + 1, active)
            }
            Expr::Subst { body, bindings } => {
                self.kind_step(depth, bindings.len())?;
                if bindings.iter().any(|binding| {
                    matches!(
                        binding.value,
                        Arg::Expr {
                            pass: dive_ir::Pass::Strict,
                            ..
                        }
                    )
                }) {
                    return Ok(ResultKind::Unknown);
                }
                self.result_kind(&ev.scope_subst(scope, expr)?, *body, depth + 1, active)
            }
            Expr::Local(local) => {
                let frames = usize::try_from(local.up)
                    .map_err(|_| EvalError::resource_limit("finite binding traversal"))?;
                self.kind_step(depth, frames.saturating_add(1))?;
                self.slot_kind(ev.scope_slot(scope, *local), depth, active)
            }
            Expr::Const(constant) => self.constant_kind(scope, *constant, depth, active),
            Expr::SubstRef {
                decl,
                primed: false,
            } => self.substitution_kind(scope, *decl, depth, active),
            Expr::Def(def) => self.call_kind(scope, *def, &[], depth, active),
            Expr::Apply {
                callee: OpRef::Def(def),
                args,
            } => self.call_kind(scope, *def, args, depth, active),
            _ => Ok(ResultKind::Unknown),
        }
    }

    fn slot_kind(
        &mut self,
        slot: SlotView,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        match slot {
            SlotView::Value(value) => Ok(value_kind(&value)),
            SlotView::Lazy(body, source) => self.result_kind(&source, body, depth + 1, active),
            _ => Ok(ResultKind::Unknown),
        }
    }

    fn constant_kind(
        &mut self,
        scope: &Scope,
        constant: ConstId,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        self.kind_step(depth, 1)?;
        match self.evaluator.constant_binding(constant) {
            Some(ConstBinding::Value(value)) => Ok(value_kind(&value)),
            // Staying conservative about configured definition values avoids
            // confusing direct-value lookup with redirected body evaluation.
            Some(ConstBinding::Def(def)) => self.call_kind(scope, def, &[], depth, active),
            None => Ok(ResultKind::Unknown),
        }
    }

    fn substitution_kind(
        &mut self,
        scope: &Scope,
        declaration: dive_ir::DeclId,
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        self.kind_step(
            depth,
            scope
                .env
                .dynamic
                .subst
                .as_ref()
                .map_or(1, |head| head.depth.saturating_add(1)),
        )?;
        match self.evaluator.scope_subst_slot(scope, declaration) {
            SlotView::Unset => match self.evaluator.ir().decl(declaration).constant {
                Some(constant) => self.constant_kind(scope, constant, depth, active),
                None => Ok(ResultKind::Unknown),
            },
            slot => self.slot_kind(slot, depth, active),
        }
    }

    fn call_kind(
        &mut self,
        scope: &Scope,
        def: dive_ir::DefId,
        args: &[Arg],
        depth: usize,
        active: &mut Vec<(ExprId, Scope)>,
    ) -> crate::Result<ResultKind> {
        self.kind_step(depth, 1)?;
        let ev = self.evaluator;
        let target = ev.admit_def(def)?;
        if ev.redirected(def) != def
            || ev.is_value_override(def)
            || target.recursive
            || target.std.is_some()
        {
            return Ok(ResultKind::Unknown);
        }
        self.kind_step(
            depth,
            target
                .frame
                .len()
                .saturating_add(args.len())
                .saturating_add(target.params.len()),
        )?;
        if target.params.iter().any(|parameter| parameter.arity != 0)
            || args.iter().any(|arg| matches!(arg, Arg::Op { .. }))
        {
            return Ok(ResultKind::Unknown);
        }
        let called = ev.scope_apply(scope, def, args)?;
        self.result_kind(&called, target.body, depth + 1, active)
    }

    fn callback_sources(&mut self, bodies: &[(ExprId, Scope)], depth: usize) -> crate::Result<()> {
        for (body, captured) in bodies {
            // Read original stored slots, never thunk result caches or provider
            // methods. Captures are checked even when their source is unused.
            self.native_storage(Some(captured), None)?;
            self.walk_data(captured, *body, depth + 1)?;
        }
        Ok(())
    }

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

    // Preserve the innermost inspected source, including a lazy actual or an
    // INSTANCE body reached through a different lexical scope. This does not
    // evaluate the expression or change the admission/work policy.
    fn walk(&mut self, scope: &Scope, expr: ExprId, depth: usize) -> crate::Result<()> {
        self.walk_inner(scope, expr, depth).map_err(|error| {
            if self.evaluator.ir().nodes().get(expr.index()).is_some() {
                error.at(expr)
            } else {
                error
            }
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit lexical binding and callback shape rules"
    )]
    fn walk_inner(&mut self, scope: &Scope, expr: ExprId, depth: usize) -> crate::Result<()> {
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
            Expr::Builtin {
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
            } | Expr::Apply {
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
            } | Expr::Enabled(_)
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
            Expr::Quant { body, bounds, .. }
            | Expr::FcnCons { body, bounds, .. }
            | Expr::RecFcn { body, bounds, .. }
            | Expr::SetMap {
                expr: body, bounds, ..
            } => {
                for bound in bounds {
                    let domain = bound.domain.ok_or_else(|| {
                        EvalError::unsupported("unbounded SelectSeq callback domain")
                    })?;
                    self.walk_data(scope, domain, depth + 1)?;
                }
                let analysis = ev.scope_binder_analysis(scope, expr)?;
                return self.walk_data(&analysis, *body, depth + 1);
            }
            Expr::SetFilter { bound, pred } => {
                let domain = bound
                    .domain
                    .ok_or_else(|| EvalError::unsupported("unbounded SelectSeq callback domain"))?;
                self.walk_data(scope, domain, depth + 1)?;
                let analysis = ev.scope_binder_analysis(scope, expr)?;
                return self.walk_data(&analysis, *pred, depth + 1);
            }
            _ => {}
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
                    SlotView::Value(_) | SlotView::AnalysisBound | SlotView::AnalysisConstant => {
                        Ok(())
                    }
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
                    Some(SlotView::AnalysisBound | SlotView::AnalysisConstant) => Ok(()),
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
                OpRef::Def(source) => {
                    let d = ev.redirected(source);
                    // Direct uses may read the stored override; redirected uses
                    // execute its target body instead. Neither source may vanish
                    // from this proof merely because the target has a value.
                    if ev.is_value_override(d) {
                        return Err(EvalError::unsupported(
                            "SelectSeq overridden definition source",
                        ));
                    }
                    let leaf = ev.recursive_leaf(source);
                    let def = ev.ir().def(d);
                    if def.recursive
                        && self.configured_depth == 0
                        && !scope.has_substitutions()
                        && leaf
                    {
                        // A state-level recursive operator is an evaluation
                        // leaf (`Evaluator::recursive_leaf`): its actuals are
                        // walked in this scope, its body is not projected.
                        for arg in args {
                            let Arg::Expr { expr, .. } = arg else {
                                return Err(EvalError::unsupported(
                                    "recursive/higher-order finite binding",
                                ));
                            };
                            self.walk_data(scope, *expr, depth + 1)?;
                        }
                        return Ok(());
                    }
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
            let concat_without_names = standard == Some(StdOp::SequencesConcat)
                && self.concat_without_names(scope, &op, args, depth + 1)?;
            if standard.is_some_and(|s| {
                s.impure()
                    || (s.creates_runtime_names() && !concat_without_names)
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
                let ceiling = self.limit.saturating_mul(64).min(100_000);
                let mut remaining = ceiling.saturating_sub(self.visits);
                let projected = ev.selectseq_projection(scope, &op, args, &mut remaining, false);
                self.visits = ceiling - remaining;
                let Some((sequence, bodies)) = projected? else {
                    return Err(EvalError::unsupported("SelectSeq nested target"));
                };
                self.walk_data(scope, sequence, depth + 1)?;
                return self.callback_sources(&bodies, depth + 1);
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

fn join_kind(left: ResultKind, right: ResultKind) -> ResultKind {
    if left == ResultKind::NonString && right == ResultKind::NonString {
        ResultKind::NonString
    } else {
        ResultKind::Unknown
    }
}

fn value_kind(value: &dive_value::Value) -> ResultKind {
    match value {
        dive_value::Value::Str(_) => ResultKind::Unknown,
        dive_value::Value::Bool(_)
        | dive_value::Value::Int(_)
        | dive_value::Value::BigInt(_)
        | dive_value::Value::Model(_)
        | dive_value::Value::Set(_)
        | dive_value::Value::Fcn(_) => ResultKind::NonString,
    }
}

fn non_string_builtin(op: BuiltinOp) -> bool {
    matches!(
        op,
        BuiltinOp::Eq
            | BuiltinOp::Neq
            | BuiltinOp::In
            | BuiltinOp::NotIn
            | BuiltinOp::Subseteq
            | BuiltinOp::Cup
            | BuiltinOp::Cap
            | BuiltinOp::SetMinus
            | BuiltinOp::PowerSet
            | BuiltinOp::BigUnion
            | BuiltinOp::Domain
            | BuiltinOp::Times
            | BuiltinOp::Boolean
            | BuiltinOp::String
            | BuiltinOp::Not
            | BuiltinOp::And
            | BuiltinOp::Or
            | BuiltinOp::Implies
            | BuiltinOp::Equiv
    )
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "private result-kind contract assertions"
)]
mod result_kind_tests {
    use super::{Guard, ResultKind};
    use crate::{Evaluator, Model, Options};
    use dive_ir::{Arg, OpRef, Pass, StdOp};
    use std::sync::Arc;

    fn fixture(expression: &str) -> (Arc<dive_ir::Ir>, Evaluator) {
        let source =
            format!("---- MODULE Kind ----\nEXTENDS TLC, Sequences\nD=={expression}\n====\n");
        let analysis =
            dive_sema::analyze_source("Kind.tla", &source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let evaluator = Evaluator::new(Model::new(ir.clone()), Options::tlc()).expect("evaluator");
        (ir, evaluator)
    }

    fn guard(evaluator: &Evaluator) -> Guard<'_> {
        Guard {
            evaluator,
            limit: 1_000,
            visits: 0,
            configured_depth: 0,
            query_allowed: false,
        }
    }

    #[test]
    fn a_constructor_type_fact_is_positive_without_executing_its_effectful_child() {
        let (ir, evaluator) = fixture("<<PrintT(0)>>");
        let def = ir.root_def("D").expect("D");
        assert!(matches!(
            guard(&evaluator)
                .result_kind(
                    &evaluator.scope(def).expect("scope"),
                    ir.def(def).body,
                    0,
                    &mut Vec::new()
                )
                .expect("result-kind metadata"),
            ResultKind::NonString
        ));
        assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
    }

    #[test]
    fn active_owned_context_cycle_is_unknown_without_discarding_its_owner() {
        let (ir, evaluator) = fixture("<<0>>");
        let def = ir.root_def("D").expect("D");
        let scope = evaluator.scope(def).expect("scope");
        let expr = ir.def(def).body;
        let mut active = vec![(expr, scope.clone())];
        assert!(matches!(
            guard(&evaluator)
                .result_kind(&scope, expr, 0, &mut active)
                .expect("bounded cycle"),
            ResultKind::Unknown
        ));
        assert_eq!(active.len(), 1);
        assert!(matches!(
            guard(&evaluator)
                .result_kind(&scope, expr, 0, &mut Vec::new())
                .expect("fresh source"),
            ResultKind::NonString
        ));
    }

    #[test]
    fn malformed_native_actual_shapes_never_gain_the_concat_exemption() {
        let (ir, evaluator) = fixture("TRUE");
        let def = ir.root_def("D").expect("D");
        let scope = evaluator.scope(def).expect("scope");
        let actual = Arg::Expr {
            expr: ir.def(def).body,
            pass: Pass::Direct,
        };
        for args in [
            Vec::new(),
            vec![actual.clone()],
            vec![actual.clone(); 3],
            vec![
                Arg::Op {
                    op: OpRef::Std(StdOp::SequencesConcat),
                    level: dive_ir::Level::Constant
                };
                2
            ],
        ] {
            assert!(
                !guard(&evaluator)
                    .concat_without_names(&scope, &OpRef::Std(StdOp::SequencesConcat), &args, 0)
                    .expect("metadata shape refusal")
            );
        }
        assert_eq!(evaluator.take_output(), [] as [std::string::String; 0]);
    }
}
